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

pub fn set_cs(segment: SegmentSelector) {
    unsafe {
        asm!(
        "push {0:r}",
        "lea {1}, [1f + rip]",
        "push {1}",
        "retfq",
        "1:",
        in(reg) segment.as_raw(),
        lateout(reg) _,
        options(preserves_flags),
        );
    }
}

pub fn set_ds(segment: SegmentSelector) {
    unsafe {
        asm!(
        "mov ds, {0:r}",
        in(reg) segment.as_raw(),
        options(nostack, preserves_flags),
        );
    }
}

pub fn set_ss(segment: SegmentSelector) {
    unsafe {
        asm!(
        "mov ss, {0:r}",
        in(reg) segment.as_raw(),
        options(nostack, preserves_flags),
        );
    }
}

pub fn set_es(segment: SegmentSelector) {
    unsafe {
        asm!(
        "mov es, {0:r}",
        in(reg) segment.as_raw(),
        options(nostack, preserves_flags),
        );
    }
}

pub fn set_fs(segment: SegmentSelector) {
    unsafe {
        asm!(
        "mov fs, {0:r}",
        in(reg) segment.as_raw(),
        options(nostack, preserves_flags),
        );
    }
}

pub fn set_gs(segment: SegmentSelector) {
    unsafe {
        asm!(
        "mov gs, {0:r}",
        in(reg) segment.as_raw(),
        options(nostack, preserves_flags),
        );
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
