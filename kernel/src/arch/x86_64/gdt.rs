use core::arch::asm;
use core::mem::size_of;

use spin::Once;

use crate::arch::registers::{set_cs, set_ds, set_es, set_fs, set_gs, set_ss};
use crate::arch::{PrivilegeLevel, SegmentSelector};
use crate::bits::Bits;
use crate::memory::address::VirtualAddress;
use crate::println;

const STACK_SIZE: usize = 8192;
pub static SELECTORS: Once<Selectors> = Once::new();
static GDT: Once<GlobalDescriptorTable> = Once::new();
static TSS: Once<TaskStateSegment> = Once::new();

pub fn init() {
    build();
    load();
    println!("Global Descriptor Table initialized.");
}

#[repr(packed)]
pub struct TaskStateSegment {
    reserved_1: u32,
    pub privilege_stack_table: [VirtualAddress; 3],
    reserved_2: u64,
    pub interrupt_stack_table: [VirtualAddress; 7],
    reserved_3: u64,
    reserved_4: u16,
    pub iomap_base: u16,
}

impl TaskStateSegment {
    fn new() -> Self {
        let mut tss = TaskStateSegment {
            reserved_1: 0,
            privilege_stack_table: [VirtualAddress::zeroed(); 3],
            reserved_2: 0,
            interrupt_stack_table: [VirtualAddress::zeroed(); 7],
            reserved_3: 0,
            reserved_4: 0,
            iomap_base: size_of::<TaskStateSegment>() as u16,
        };
        tss.privilege_stack_table[0] = {
            let stack_start = VirtualAddress::from_ptr(unsafe {
                static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
                &STACK
            });
            stack_start + STACK_SIZE as u64
        };
        tss
    }
}

struct GlobalDescriptorTable {
    table: [u64; 8],
    next_free: usize,
}

impl GlobalDescriptorTable {
    const DEFAULT_DESCRIPTOR: u64 = 1 << 44 | 1 << 47 | 1 << 41 | 1 << 40;
    fn new() -> Self {
        Self {
            table: [0; 8],
            next_free: 1,
        }
    }

    fn load(&self) {
        let ptr = GlobalDescriptorTablePointer {
            limit: (self.table.len() * size_of::<u64>() - 1) as u16,
            base: self.table.as_ptr() as u64,
        };

        unsafe {
            asm!("lgdt [{0}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        }
    }

    fn add_kernel_code_entry(&mut self) -> SegmentSelector {
        let descriptor = Self::DEFAULT_DESCRIPTOR | 1 << 43 | 1 << 53;
        self.add_entry(descriptor, PrivilegeLevel::Ring0)
    }

    fn add_kernel_data_entry(&mut self) -> SegmentSelector {
        let descriptor = Self::DEFAULT_DESCRIPTOR | 1 << 54;
        self.add_entry(descriptor, PrivilegeLevel::Ring0)
    }

    fn add_user_code_entry(&mut self) -> SegmentSelector {
        let descriptor = Self::DEFAULT_DESCRIPTOR | 1 << 43 | 1 << 53 | 3 << 45;
        self.add_entry(descriptor, PrivilegeLevel::Ring3)
    }

    fn add_user_data_entry(&mut self) -> SegmentSelector {
        let descriptor = Self::DEFAULT_DESCRIPTOR | 1 << 54 | 3 << 45;
        self.add_entry(descriptor, PrivilegeLevel::Ring3)
    }

    fn add_tss_entry(&mut self, tss: *const TaskStateSegment) -> SegmentSelector {
        let tss_ptr = tss as u64;
        let mut low_descriptor = 1u64 << 47;

        low_descriptor.set_bits(16..40, tss_ptr.get_bits(0..24));
        low_descriptor.set_bits(56..64, tss_ptr.get_bits(24..32));
        low_descriptor.set_bits(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        low_descriptor.set_bits(40..44, 0b1001);

        let mut high_descriptor = 0;
        high_descriptor.set_bits(0..32, tss_ptr.get_bits(32..64));

        let segment = self.add_entry(low_descriptor, PrivilegeLevel::Ring0);
        self.add_entry(high_descriptor, PrivilegeLevel::Ring0);
        segment
    }
    fn add_entry(&mut self, descriptor: u64, privilege_level: PrivilegeLevel) -> SegmentSelector {
        let index = self.next_free;
        self.table[index] = descriptor;
        self.next_free += 1;
        SegmentSelector::new(index as u16, privilege_level)
    }
}

#[repr(packed)]
struct GlobalDescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    tss: SegmentSelector,
}

fn build() {
    TSS.call_once(|| TaskStateSegment::new());

    let mut gdt = GlobalDescriptorTable::new();

    let kernel_code_selector = gdt.add_kernel_code_entry();
    let kernel_data_selector = gdt.add_kernel_data_entry();
    let user_data_selector = gdt.add_user_data_entry();
    let user_code_selector = gdt.add_user_code_entry();
    let tss_selector = gdt.add_tss_entry(TSS.get().unwrap());

    let selectors = Selectors {
        kernel_code: kernel_code_selector,
        kernel_data: kernel_data_selector,
        user_code: user_code_selector,
        user_data: user_data_selector,
        tss: tss_selector,
    };

    SELECTORS.call_once(|| selectors);
    GDT.call_once(|| gdt);
}

fn load() {
    let gdt = GDT.get().expect("GDT not initialized.");
    gdt.load();

    let selectors = SELECTORS.get().expect("GDT selectors not initialized.");
    unsafe {
        let code_selector = selectors.kernel_code;
        let data_selector = selectors.kernel_data;

        set_cs(code_selector);
        set_ds(data_selector);
        set_es(data_selector);
        set_fs(data_selector);
        set_gs(data_selector);
        set_ss(data_selector);

        // Load the TSS
        asm!("ltr {0:x}", in(reg) selectors.tss.as_raw(), options(nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_gdt_initialized() {
        assert!(GDT.is_completed());
        let gdt = GDT.get().unwrap();
        assert_eq!(gdt.next_free, 7);
    }

    #[test_case]
    fn test_gdt_selectors_initialized() {
        assert!(SELECTORS.is_completed());
        assert!(TSS.is_completed());

        let selectors = SELECTORS.get().unwrap();
        assert_eq!(selectors.kernel_code.as_raw(), 0x8);
        assert_eq!(selectors.kernel_data.as_raw(), 0x10);
        assert_eq!(selectors.user_code.as_raw(), 0x23);
        assert_eq!(selectors.user_data.as_raw(), 0x1B);
        assert_eq!(selectors.tss.as_raw(), 0x28);
    }
}
