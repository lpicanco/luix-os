use core::arch::asm;

use crate::arch::SegmentSelector;
use crate::memory::address::PhysicalAddress;

pub(crate) fn read_cs() -> SegmentSelector {
    let segment: u16;
    unsafe {
        asm!(concat!("mov {0:x}, ", "cs"),
            out(reg) segment,
            options(nomem, nostack, preserves_flags));
        return SegmentSelector::from_raw(segment);
    }
}

pub(crate) fn read_cr2() -> PhysicalAddress {
    let cr2: u64;
    unsafe {
        asm!("mov {}, cr2",
        out(reg) cr2,
        options(nomem, nostack, preserves_flags));
    }
    return PhysicalAddress::new(cr2)
}


pub(crate) fn read_cr3() -> PhysicalAddress {
    let cr3: u64;
    unsafe {
        asm!("mov {}, cr3",
            out(reg) cr3,
            options(nomem, nostack, preserves_flags));
    }
    return PhysicalAddress::new(cr3 & 0x_000f_ffff_ffff_f000) // Mask out the lower 12 bits
}
