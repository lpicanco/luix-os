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
