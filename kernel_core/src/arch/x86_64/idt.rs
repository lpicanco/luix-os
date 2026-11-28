use core::arch::asm;
use core::mem::size_of;

use crate::arch::x86_64::gdt::DOUBLE_FAULT_IST_INDEX;
use crate::arch::x86_64::registers::read_cs;
use crate::arch::{InterruptFrame, PrivilegeLevel, SegmentSelector};
use crate::bits::Bits;

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
        // SAFETY: The core owns the initialized static IDT for the entire CPU lifetime.
        unsafe {
            asm!("lidt [{}]", in(reg) &idt, options(readonly, nostack, preserves_flags));
        }
    }

    pub fn set_division_error_handler(&mut self, handler: Handler) {
        self.set_handler(0, handler);
    }

    pub fn set_overflow_handler(&mut self, handler: Handler) {
        self.set_handler(4, handler);
    }
    pub fn set_invalid_opcode_handler(&mut self, handler: Handler) {
        self.set_handler(6, handler);
    }

    pub fn set_double_fault_handler(&mut self, handler: HandlerWithErrorCode) {
        self.entries[8] = Entry::new(handler as u64);
        // IDT gates encode the zero-based TSS array index plus one.
        self.entries[8]
            .options
            .0
            .write_bits(0..3, (DOUBLE_FAULT_IST_INDEX + 1) as u16);
    }

    pub fn set_stack_segment_fault_handler(&mut self, handler: HandlerWithErrorCode) {
        self.entries[12] = Entry::new(handler as u64);
    }

    pub fn set_general_protection_fault_handler(&mut self, handler: HandlerWithErrorCode) {
        self.entries[13] = Entry::new(handler as u64);
    }

    pub fn set_page_fault_handler(&mut self, handler: HandlerWithErrorCode) {
        self.entries[14] = Entry::new(handler as u64);
    }

    pub fn set_handler(&mut self, index: usize, handler: Handler) {
        self.entries[index] = Entry::new(handler as u64);
    }

    #[cfg(test)]
    pub(super) fn assert_gate(&self, vector: usize, address: u64, ist: u16) {
        let entry = &self.entries[vector];
        let offset = (u64::from(entry.pointer_high) << 32)
            | (u64::from(entry.pointer_middle) << 16)
            | u64::from(entry.pointer_low);
        assert_eq!(offset, address);
        assert_eq!(entry.options.0, 0x8e00 | ist);
    }

    pub fn set_privilege_level(&mut self, index: usize, privilege_level: PrivilegeLevel) {
        self.entries[index]
            .options
            .0
            .write_bits(13..15, privilege_level as u16);
    }
}

type Handler = extern "x86-interrupt" fn(InterruptFrame);
type HandlerWithErrorCode = extern "x86-interrupt" fn(InterruptFrame, u64);

impl Entry {
    fn new(addr: u64) -> Self {
        Entry {
            gdt_selector: read_cs().0,
            pointer_low: addr as u16,
            pointer_middle: (addr >> 16) as u16,
            pointer_high: (addr >> 32) as u32,
            options: EntryOptions::new(),
            reserved: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: u16,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32, // always zero
}

impl Entry {
    pub const fn default() -> Self {
        let options = 0;
        Entry {
            pointer_low: 0,
            gdt_selector: SegmentSelector::new(0, PrivilegeLevel::Ring0).0,
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

#[repr(transparent)]
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
        options.set_present(true);
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
