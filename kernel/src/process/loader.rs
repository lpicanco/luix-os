use alloc::vec::Vec;
use core::ptr;

use spin::RwLock;

use crate::arch::gdt::SELECTORS;
use crate::arch::memory::paging::{Page, PageIter, PAGE_SIZE};
use crate::arch::{InterruptFrame, Registers};
use crate::memory::address::VirtualAddress;
use crate::memory::allocator::FRAME_ALLOCATOR;
use crate::memory::MEMORY_MAPPER;
use crate::process::elf::{ElfFile, ProgramHeaderType};
use crate::process::Process;

const PROCESS_START: u64 = 0xF00D_C0DE_000;
const PROCESS_SIZE: u64 = 10 * PAGE_SIZE;
const IMAGE_SIZE: u64 = PROCESS_SIZE - PAGE_SIZE;

static PROCESS_LIST: RwLock<Vec<Process>> = RwLock::new(Vec::new());

pub(crate) fn spawn(
    elf_file: &ElfFile,
    interrupt_frame: &mut InterruptFrame,
    regs: &mut Registers,
) -> Result<(), ()> {
    let segments = elf_file
        .program_headers
        .iter()
        .filter(|header| matches!(header.p_type, ProgramHeaderType::Load));
    let entry_point = elf_file.entry_point();
    if !segments
        .clone()
        .any(|header| header.contains_entry(entry_point))
        || segments.clone().any(|header| {
            header.p_filesz > header.p_memsz
                || header
                    .p_vaddr
                    .checked_add(header.p_memsz)
                    .is_none_or(|end| end > IMAGE_SIZE)
                || elf_file.data(header).is_none()
        })
    {
        return Err(());
    }

    let mut processes = PROCESS_LIST.write();
    // Spawn is synchronous, so the nesting depth can be reused as the address slot.
    let base_address = PROCESS_START + processes.len() as u64 * PROCESS_SIZE;
    let mapper = MEMORY_MAPPER.get().unwrap();
    for (index, page) in process_pages(base_address).enumerate() {
        let frame = FRAME_ALLOCATOR.get().unwrap().lock().allocate_frame();
        let Some(frame) = frame else {
            release_pages(process_pages(base_address).take(index));
            return Err(());
        };
        mapper.map_page(page, frame, true, true);
        unsafe { ptr::write_bytes(page.start_address.as_mut_ptr::<u8>(), 0, PAGE_SIZE as usize) };
    }

    for header in segments {
        let data = elf_file.data(header).expect("validated ELF segment");
        let destination = VirtualAddress::new(base_address + header.p_vaddr);
        unsafe { ptr::copy_nonoverlapping(data.as_ptr(), destination.as_mut_ptr(), data.len()) };
    }

    processes.push(Process {
        base_address,
        parent_frame: *interrupt_frame,
        parent_registers: *regs,
    });

    let selectors = SELECTORS.get().expect("GDT not initialized");
    *interrupt_frame = InterruptFrame {
        instruction_pointer: base_address + entry_point,
        code_segment: u64::from(selectors.user_code.as_raw()),
        cpu_flags: 0x202,
        // Match the ABI's stack alignment at function entry.
        stack_pointer: base_address + PROCESS_SIZE - 8,
        stack_segment: u64::from(selectors.user_data.as_raw()),
    };
    *regs = Registers::default();
    Ok(())
}

pub(crate) fn exit(interrupt_frame: &mut InterruptFrame, regs: &mut Registers) -> Result<(), ()> {
    let mut processes = PROCESS_LIST.write();
    let process = processes.pop().ok_or(())?;
    release_pages(process_pages(process.base_address));
    *interrupt_frame = process.parent_frame;
    *regs = process.parent_registers;
    Ok(())
}

fn process_pages(base_address: u64) -> PageIter {
    let start = Page::containing_address(VirtualAddress::new(base_address));
    let end = Page::containing_address(VirtualAddress::new(base_address + PROCESS_SIZE - 1));
    Page::range_inclusive(start, end)
}

fn release_pages(pages: impl Iterator<Item = Page>) {
    let mapper = MEMORY_MAPPER.get().unwrap();
    let mut allocator = FRAME_ALLOCATOR.get().unwrap().lock();
    for page in pages {
        let frame = mapper.unmap_page(page).expect("process page not mapped");
        allocator.deallocate_frames(frame.start_address, PAGE_SIZE as usize);
    }
}

#[cfg(test)]
mod tests {
    use kernel_api::syscall::SPAWN;

    use super::*;

    #[test_case]
    fn test_spawn_exit_and_resume() {
        let missing = "/boot/MISSING.TST";
        assert_eq!(
            unsafe { kernel_api::make_syscall!(SPAWN, missing.as_ptr(), missing.len()) },
            1
        );

        let path = "/boot/spawn.tst";
        let mut frames_after_first_run = None;
        for _ in 0..2 {
            assert_eq!(
                unsafe { kernel_api::make_syscall!(SPAWN, path.as_ptr(), path.len()) },
                0
            );

            assert!(PROCESS_LIST.read().is_empty());
            let mapper = MEMORY_MAPPER.get().unwrap();
            for base in [PROCESS_START, PROCESS_START + PROCESS_SIZE] {
                for page in process_pages(base) {
                    assert_eq!(mapper.translate_addr(page.start_address), None);
                }
            }

            let allocated = FRAME_ALLOCATOR
                .get()
                .unwrap()
                .lock()
                .allocated_frame_count();
            // The first run can allocate page tables that the next run will reuse.
            if let Some(first_run) = frames_after_first_run {
                assert_eq!(allocated, first_run);
            }
            frames_after_first_run = Some(allocated);
        }
    }
}
