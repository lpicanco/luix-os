#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(all(test, target_os = "none"), no_main)]
#![cfg_attr(target_os = "none", feature(custom_test_frameworks))]
#![cfg_attr(target_os = "none", test_runner(crate::test_runner))]
#![cfg_attr(target_os = "none", reexport_test_harness_main = "test_main")]
// Host builds only compile the hardware-independent subset for unit tests,
// so code that only the kernel calls looks unused there.
#![cfg_attr(not(target_os = "none"), allow(dead_code, unused_imports))]

extern crate alloc;

// Replaces the standard #[test] in this crate.
// Plain #[test] runs on the host and in QEMU, and #[test(target = "qemu")] runs only in QEMU.
// See tools/test-macros.
#[cfg(test)]
#[macro_use]
extern crate test_macros;

#[cfg(target_os = "none")]
pub use kernel_core::{print, println, test_runner, trace};
pub(crate) mod bits;
mod drivers;
mod process;
#[cfg(target_os = "none")]
mod syscall;
pub mod utils;

#[cfg(target_os = "none")]
pub fn init() {
    kernel_core::boot::initialize(syscall::dispatcher);
    drivers::init();
}

#[cfg(target_os = "none")]
pub fn run() -> ! {
    init();
    kernel_core::user::launch("/boot/init");
    loop {
        kernel_core::halt();
    }
}

#[cfg(all(test, target_os = "none"))]
#[kernel_core::entry]
fn tests() -> ! {
    init();
    test_main();
    loop {
        kernel_core::halt();
    }
}
