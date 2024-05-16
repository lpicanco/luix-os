pub(crate) mod apic;
mod idt;
pub(crate) mod interrupt;
pub(crate) mod memory;
pub(crate) mod port;
pub(crate) mod registers;
mod serial_writer;
pub(crate) mod instructions;

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
}

#[repr(u8)]
#[allow(dead_code)]
pub enum PrivilegeLevel {
    Ring0 = 0,
    Ring3 = 3,
}
