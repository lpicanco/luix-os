use core::arch::asm;

/// Halt the CPU until the next interrupt.
pub fn halt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags))
    }
}