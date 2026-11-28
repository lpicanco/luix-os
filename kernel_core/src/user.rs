//! Owned process mappings and a restricted view of syscall return state.
//! Synchronous process nesting on one CPU. Separate page tables follow in LUIX-20.
use crate::arch::memory::paging::{Page, PAGE_SIZE};
use crate::arch::{InterruptFrame, Registers};
use crate::memory::address::VirtualAddress;
use crate::memory::{allocator::FRAME_ALLOCATOR, MEMORY_MAPPER};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::{arch::asm, ptr};
use spin::{Mutex, Once};

const PROCESS_START: u64 = 0xF00D_C0DE_000;
const PAGES: usize = 10;
const SIZE: usize = PAGES * PAGE_SIZE as usize;
static SLOTS: AtomicU64 = AtomicU64::new(0);
static PROCESSES: Mutex<Vec<Process>> = Mutex::new(Vec::new());
static DISPATCH: Once<fn(&mut SyscallContext<'_>) -> usize> = Once::new();
static KERNEL_ARGUMENT: Mutex<Option<(usize, usize)>> = Mutex::new(None);
static LAUNCH_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Invalid user range, exhausted resources or invalid transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserError;

/// A unique, zeroed process image and stack. Ownership controls unmapping.
pub struct ProcessMemory {
    slot: u32,
    pages: usize,
}
impl ProcessMemory {
    /// Maximum loadable image size, excluding the reserved stack page.
    pub const IMAGE_SIZE: usize = SIZE - PAGE_SIZE as usize;
    fn base(&self) -> u64 {
        PROCESS_START + u64::from(self.slot) * SIZE as u64
    }
    /// Allocate a fresh private slot. Partial failures release all acquired pages.
    pub fn allocate() -> Result<Self, UserError> {
        let mapper = MEMORY_MAPPER.get().ok_or(UserError)?;
        let allocator = FRAME_ALLOCATOR.get().ok_or(UserError)?;
        let mut used = SLOTS.load(Ordering::Acquire);
        let slot = loop {
            let slot = (!used).trailing_zeros();
            if slot == 64 {
                return Err(UserError);
            }
            match SLOTS.compare_exchange_weak(
                used,
                used | (1 << slot),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break slot,
                Err(next) => used = next,
            }
        };
        let mut memory = Self { slot, pages: 0 };
        for i in 0..PAGES {
            let frame = allocator.lock().allocate_frame().ok_or(UserError)?;
            let address = VirtualAddress::new(memory.base() + i as u64 * PAGE_SIZE);
            mapper.map_page(Page::containing_address(address), frame, true, true);
            memory.pages += 1;
            // SAFETY: this unique slot owns the newly mapped frame and is not executable yet.
            unsafe {
                ptr::write_bytes(address.as_mut_ptr::<u8>(), 0, PAGE_SIZE as usize);
            }
        }
        Ok(memory)
    }
    /// Copy initialized bytes into this owned image. No arbitrary address is accepted.
    pub fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), UserError> {
        if offset
            .checked_add(data.len())
            .is_none_or(|end| end > Self::IMAGE_SIZE)
        {
            return Err(UserError);
        }
        // SAFETY: all pages of this exclusive region are live. The checked destination
        // is disjoint from the caller's ordinary Rust allocation. Running regions are moved away.
        unsafe {
            ptr::copy_nonoverlapping(
                data.as_ptr(),
                (self.base() as usize + offset) as *mut u8,
                data.len(),
            );
        }
        Ok(())
    }
}
impl Drop for ProcessMemory {
    fn drop(&mut self) {
        let mapper = MEMORY_MAPPER
            .get()
            .expect("owned mapping requires initialized memory");
        for i in 0..self.pages {
            let page =
                Page::containing_address(VirtualAddress::new(self.base() + i as u64 * PAGE_SIZE));
            let frame = mapper
                .unmap_page(page)
                .expect("owned process page remains mapped");
            FRAME_ALLOCATOR
                .get()
                .unwrap()
                .lock()
                .deallocate_frames(frame.start_address, PAGE_SIZE as usize);
        }
        SLOTS.fetch_and(!(1 << self.slot), Ordering::Release);
    }
}
struct Process {
    memory: ProcessMemory,
    parent_frame: InterruptFrame,
    parent_registers: Registers,
}

/// A syscall context issued by the core with private privileged fields.
pub struct SyscallContext<'a> {
    frame: &'a mut InterruptFrame,
    registers: &'a mut Registers,
}
impl SyscallContext<'_> {
    /// System call number supplied by the trapped execution context.
    pub fn number(&self) -> usize {
        self.registers.rax
    }
    /// A scalar argument. Scalars are never implicitly treated as trusted pointers.
    pub fn arg(&self, index: usize) -> usize {
        match index {
            0 => self.registers.rdi,
            1 => self.registers.rsi,
            _ => 0,
        }
    }
    /// Copy at most 4096 bytes from the current process, or the core's retained launch argument.
    pub fn copy_argument(&self, address: usize, len: usize) -> Result<Vec<u8>, UserError> {
        if len > 4096 {
            return Err(UserError);
        }
        let processes = PROCESSES.lock();
        let allowed = if self.frame.code_segment & 3 == 0 {
            KERNEL_ARGUMENT
                .lock()
                .is_some_and(|(start, size)| contains(start, size, address, len))
        } else {
            processes
                .last()
                .is_some_and(|p| contains(p.memory.base() as usize, SIZE, address, len))
        };
        if !allowed {
            return Err(UserError);
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(len).map_err(|_| UserError)?;
        bytes.resize(len, 0);
        if len != 0 {
            // SAFETY: the process lock retains mapped pages for the whole copy. The
            // single CPU cannot run user code concurrently. A launch argument is a live &str.
            // No reference into user memory is formed or retained.
            unsafe {
                ptr::copy_nonoverlapping(address as *const u8, bytes.as_mut_ptr(), len);
            }
        }
        Ok(bytes)
    }
    /// Enter a region owned by the core. Privilege selectors and flags are fixed here.
    pub fn enter(&mut self, memory: ProcessMemory, entry: u64) -> Result<(), UserError> {
        if entry >= ProcessMemory::IMAGE_SIZE as u64 || memory.pages != PAGES {
            return Err(UserError);
        }
        let selectors = crate::arch::gdt::SELECTORS.get().ok_or(UserError)?;
        let base = memory.base();
        let mut processes = PROCESSES.lock();
        processes.try_reserve(1).map_err(|_| UserError)?;
        processes.push(Process {
            memory,
            parent_frame: *self.frame,
            parent_registers: *self.registers,
        });
        *self.frame = InterruptFrame {
            instruction_pointer: base + entry,
            code_segment: selectors.user_code.as_raw().into(),
            cpu_flags: 0x202,
            stack_pointer: base + SIZE as u64 - 8,
            stack_segment: selectors.user_data.as_raw().into(),
        };
        *self.registers = Registers::default();
        Ok(())
    }
    /// Retire the current process and restore only a previously saved trusted context.
    pub fn exit(&mut self) -> Result<(), UserError> {
        let process = PROCESSES.lock().pop().ok_or(UserError)?;
        *self.frame = process.parent_frame;
        *self.registers = process.parent_registers;
        drop(process);
        Ok(())
    }
}
fn contains(start: usize, size: usize, address: usize, len: usize) -> bool {
    address >= start
        && address
            .checked_add(len)
            .zip(start.checked_add(size))
            .is_some_and(|(end, limit)| end <= limit)
}
pub(crate) fn install(dispatch: fn(&mut SyscallContext<'_>) -> usize) {
    DISPATCH.call_once(|| dispatch);
}
pub(crate) fn dispatch(frame: &mut InterruptFrame, registers: &mut Registers) -> usize {
    DISPATCH
        .get()
        .map_or(1, |f| f(&mut SyscallContext { frame, registers }))
}
/// Launch from kernel policy through the existing synchronous trap/return path.
/// The borrowed path remains alive until the process tree returns.
pub fn launch(path: &str) -> usize {
    if DISPATCH.get().is_none() || path.len() > 4096 || LAUNCH_ACTIVE.swap(true, Ordering::AcqRel) {
        return 1;
    }
    *KERNEL_ARGUMENT.lock() = Some((path.as_ptr() as usize, path.len()));
    let result;
    // SAFETY: int 0x80 is installed by boot. Registers follow its ABI. The retained
    // immutable argument is checked separately from untrusted user pointers.
    unsafe {
        asm!("int 0x80", inlateout("rax") kernel_api::syscall::SPAWN => result,
        in("rdi") path.as_ptr(), in("rsi") path.len());
    }
    *KERNEL_ARGUMENT.lock() = None;
    LAUNCH_ACTIVE.store(false, Ordering::Release);
    result
}
/// Number of active synchronous process contexts, useful for diagnostics.
pub fn active_process_count() -> usize {
    PROCESSES.lock().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_reject_overflow_and_foreign_memory() {
        assert!(contains(0x1000, 4096, 0x1000, 4096));
        assert!(!contains(0x1000, 4096, 0xfff, 1));
        assert!(!contains(0x1000, 4096, 0x1fff, 2));
        assert!(!contains(0x1000, 4096, usize::MAX, 2));
    }
    #[test(target = "qemu")]
    fn owned_regions_are_unique_and_checked() {
        let mut first = ProcessMemory::allocate().unwrap();
        let second = ProcessMemory::allocate().unwrap();
        assert_ne!(first.base(), second.base());
        assert_eq!(first.write(usize::MAX, &[1]), Err(UserError));
        assert_eq!(first.write(ProcessMemory::IMAGE_SIZE, &[1]), Err(UserError));
        first.write(0, b"owned").unwrap();
        let base = first.base();
        drop(first);
        for i in 0..PAGES {
            assert_eq!(
                MEMORY_MAPPER
                    .get()
                    .unwrap()
                    .translate_addr(VirtualAddress::new(base + i as u64 * PAGE_SIZE)),
                None
            );
        }
        let reused = ProcessMemory::allocate().unwrap();
        assert_eq!(reused.base(), base);
        // Page tables remain owned by the core. Payload frames must return on every drop.
        drop(reused);
        let baseline = FRAME_ALLOCATOR
            .get()
            .unwrap()
            .lock()
            .allocated_frame_count();
        for _ in 0..4 {
            drop(ProcessMemory::allocate().unwrap());
        }
        assert_eq!(
            FRAME_ALLOCATOR
                .get()
                .unwrap()
                .lock()
                .allocated_frame_count(),
            baseline
        );
    }
}
