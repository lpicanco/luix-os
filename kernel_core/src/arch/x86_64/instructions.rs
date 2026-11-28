use core::arch::asm;

/// Halt the CPU until the next interrupt.
pub fn halt() {
    // SAFETY: This privileged instruction runs in kernel mode and does not access Rust memory.
    unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) }
}
