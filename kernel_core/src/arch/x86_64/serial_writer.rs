use crate::arch::x86_64::port;
use crate::serial::{Serial, SerialWriter};

const PORT: u16 = 0x3F8; // COM1

impl SerialWriter for Serial {
    // TODO: Disable interrupts
    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            write_byte(byte);
        }
    }
}

fn write_byte(byte: u8) {
    port::write(PORT, byte);
}
