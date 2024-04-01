use core::arch::asm;

use crate::arch::SegmentSelector;

pub(crate) fn read_cs() -> SegmentSelector {
    let segment: u16;
    unsafe {
        asm!(concat!("mov {0:x}, ", "cs"),
            out(reg) segment,
            options(nomem, nostack, preserves_flags));
        return SegmentSelector::from_raw(segment);
    }
}
