//! Trusted x86_64 mechanisms. Raw memory and hardware access are private.
//! The included DMA drivers remain trusted until the LUIX-47 migration.
#![no_std]
#![deny(
    missing_docs,
    unsafe_op_in_unsafe_fn,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block
)]
#![cfg_attr(all(test, target_os = "none"), no_main)]
#![cfg_attr(
    target_os = "none",
    feature(abi_x86_interrupt, allocator_ext, custom_test_frameworks)
)]
#![cfg_attr(all(test, target_os = "none"), test_runner(crate::test_runner))]
#![cfg_attr(
    all(test, target_os = "none"),
    reexport_test_harness_main = "test_main"
)]
#![cfg_attr(not(target_os = "none"), allow(dead_code, unused_imports))]

extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate test_macros;

mod acpi;
mod arch;
mod bits;
#[cfg(target_os = "none")]
mod display;
#[cfg(target_os = "none")]
mod legacy_drivers;
mod memory;
#[cfg(target_os = "none")]
mod serial;

pub mod block;
#[cfg(target_os = "none")]
pub mod boot;
#[cfg(target_os = "none")]
pub mod print;
#[cfg(target_os = "none")]
pub mod test;
#[cfg(target_os = "none")]
pub mod user;
pub use kernel_core_macros::entry;

/// Wait for an interrupt without exposing privileged instructions.
#[cfg(target_os = "none")]
pub fn halt() {
    arch::instructions::halt();
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // SAFETY: panic terminates this CPU. Preventing interrupt reentry is required before diagnostics.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    // Avoid taking locks that may already be held by the panicking code.
    use core::fmt::Write;
    let _ = writeln!(crate::serial::Serial {}, "Panic! {info}");
    if cfg!(any(test, feature = "test-support")) {
        test::exit_qemu(test::QemuExitCode::Failed);
    }
    // SAFETY: stopping this CPU cannot expose memory to safe callers.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }
    loop {
        halt();
    }
}

/// Guest test harness shared by the trusted and safe crates.
#[cfg(target_os = "none")]
pub fn test_runner(tests: &[&dyn test::Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    println!("All tests passed!");
    test::exit_qemu(test::QemuExitCode::Success);
}

#[cfg(all(test, target_os = "none"))]
#[entry]
fn core_tests() -> ! {
    boot::initialize(|_| 1);
    test_main();
    loop {
        halt();
    }
}
