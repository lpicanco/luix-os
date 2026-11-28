//! Guest test diagnostics and QEMU completion signaling.
use crate::{print, println};

/// A guest test callable by the custom harness.
pub trait Testable {
    /// Run the test and report its name and outcome.
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{} ... ", core::any::type_name::<T>());
        self();
        println!("\x1b[92mPASSED\x1b[0m");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
/// Exit codes recognized by the QEMU test runner.
pub enum QemuExitCode {
    /// All guest tests passed.
    Success = 0x10,
    /// A guest test failed or panicked.
    Failed = 0x11,
}
/// Signal a test result to the emulator debug-exit device.
pub fn exit_qemu(exit_code: QemuExitCode) {
    use crate::arch::port;
    port::write(0xf4, exit_code as u8);
}
