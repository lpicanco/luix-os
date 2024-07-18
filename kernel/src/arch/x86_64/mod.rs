use core::fmt;

pub(crate) mod apic;
pub mod gdt;
mod idt;
pub(crate) mod instructions;
pub(crate) mod interrupt;
pub(crate) mod memory;
pub(crate) mod port;
pub(crate) mod registers;
mod serial_writer;

#[repr(C, packed)]
#[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct SegmentSelector(u16);

impl SegmentSelector {
    pub const fn new(index: u16, rpl: PrivilegeLevel) -> SegmentSelector {
        SegmentSelector(index << 3 | (rpl as u16))
    }

    pub const fn from_raw(bits: u16) -> SegmentSelector {
        SegmentSelector(bits)
    }

    pub fn as_raw(&self) -> u16 {
        self.0
    }
}

#[repr(u8)]
#[allow(dead_code)]
pub enum PrivilegeLevel {
    Ring0 = 0,
    Ring3 = 3,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Registers {
    pub r15: usize,
    pub r14: usize,
    pub r13: usize,
    pub r12: usize,
    pub r11: usize,
    pub r10: usize,
    pub r9: usize,
    pub r8: usize,
    pub rdi: usize,
    pub rsi: usize,
    pub rdx: usize,
    pub rcx: usize,
    pub rbx: usize,
    pub rax: usize,
    pub rbp: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InterruptFrame {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: u64,
    pub stack_segment: u64,
}

impl fmt::Display for InterruptFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InterruptFrame\n")?;
        write!(f, "\tip: {:#x}\n", self.instruction_pointer)?;
        write!(f, "\tcode_segment: {:#x}\n", self.code_segment)?;
        write!(f, "\tcpu_flags: {:#x}\n", self.cpu_flags)?;
        write!(f, "\tstack_pointer: {:#x}\n", self.stack_pointer)?;
        write!(f, "\tstack_segment: {:#x}\n", self.stack_segment)
    }
}
