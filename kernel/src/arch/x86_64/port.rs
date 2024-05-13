use core::arch::asm;

pub(crate) fn write(port: u16, byte: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") byte,
        );
    }
}

pub(crate) fn write_32(port: u16, value: u32) {
    unsafe {
        asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        );
    }
}

pub(crate) fn read_32(port: u16) -> u32 {
    let value: u32;
    unsafe {
        asm!(
        "in eax, dx",
        out("eax") value,
        in("dx") port,
        );
    }
    value
}
