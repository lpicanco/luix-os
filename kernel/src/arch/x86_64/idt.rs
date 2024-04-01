use core::arch::asm;
use core::mem::size_of;

use crate::arch::{PrivilegeLevel, SegmentSelector};
use crate::arch::x86_64::registers::read_cs;

#[repr(C)]
#[repr(align(16))]
pub(crate) struct InterruptDescriptorTable {
    entries: [Entry; 256],
}

impl InterruptDescriptorTable {
    pub fn new() -> Self {
        InterruptDescriptorTable {
            entries: [Entry::default(); 256],
        }
    }

    pub fn load(&'static self) {
        let idt = InterruptDescriptorTablePointer {
            base: self as *const _ as u64,
            size: (size_of::<Self>() - 1) as u16,
        };
        unsafe {
            asm!("lidt [{}]", in(reg) &idt, options(readonly, nostack, preserves_flags));
        }
    }

    pub fn set_divide_by_zero_handler(&mut self, handler: Handler) {
        self.set_handler(0, handler);
    }
    pub fn set_handler(&mut self, index: usize, handler: Handler) {
        self.entries[index] = Entry::new(handler);
    }
}

type Handler = extern "x86-interrupt" fn();

impl Entry {
    fn new(handler: Handler) -> Self {
        let addr = handler as u64;

        Entry {
            gdt_selector: read_cs(),
            pointer_low: addr as u16,
            pointer_middle: (addr >> 16) as u16,
            pointer_high: (addr >> 32) as u32,
            options: EntryOptions::new(),
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: SegmentSelector,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u8, // always zero
}

impl Entry {
    pub const fn default() -> Self {
        let options = 0;
        Entry {
            pointer_low: 0,
            gdt_selector: SegmentSelector::new(0, PrivilegeLevel::Ring0),
            pointer_middle: 0,
            pointer_high: 0,
            options: EntryOptions(options),
            reserved: 0,
        }
    }
}

#[repr(C, packed(2))]
pub struct InterruptDescriptorTablePointer {
    pub size: u16,
    pub base: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(pub u16);

impl EntryOptions {
    fn minimal() -> Self {
        let mut options = 0;
        options |= 0b111 << 9; // 'must-be-one' bits
        EntryOptions(options)
    }

    pub(crate) fn new() -> Self {
        let mut options = Self::minimal();
        options.set_present(true).disable_interrupts(true);
        options
    }

    pub fn set_present(&mut self, present: bool) -> &mut Self {
        self.0 |= (present as u16) << 15;
        self
    }

    pub fn disable_interrupts(&mut self, disable: bool) -> &mut Self {
        self.0 |= (!disable as u16) << 8;
        self
    }
}
