#![no_std]
#![cfg_attr(all(test, target_os = "none"), no_main)]
#![cfg_attr(target_os = "none", feature(abi_x86_interrupt))]
#![cfg_attr(target_os = "none", feature(const_mut_refs))]
#![cfg_attr(target_os = "none", feature(custom_test_frameworks))]
#![cfg_attr(target_os = "none", feature(allocator_api))]
#![cfg_attr(target_os = "none", feature(new_uninit))]
#![cfg_attr(target_os = "none", feature(strict_provenance))]
#![cfg_attr(target_os = "none", feature(maybe_uninit_as_bytes))]
#![cfg_attr(target_os = "none", test_runner(crate::test_runner))]
#![cfg_attr(target_os = "none", reexport_test_harness_main = "test_main")]

#[cfg(target_os = "none")]
extern crate alloc;

// Replaces the standard #[test] in this crate.
// Plain #[test] runs on the host and in QEMU, and #[test(target = "qemu")] runs only in QEMU.
// See tools/test-macros.
#[cfg(test)]
#[macro_use]
extern crate test_macros;

#[cfg(target_os = "none")]
use core::arch::asm;
#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
mod acpi;
#[cfg(target_os = "none")]
mod arch;
#[cfg(target_os = "none")]
mod display;
#[cfg(target_os = "none")]
mod drivers;
#[cfg(target_os = "none")]
mod memory;
#[cfg(target_os = "none")]
pub mod print;
#[cfg(target_os = "none")]
mod serial;
pub(crate) mod bits;
#[cfg(target_os = "none")]
mod process;
#[cfg(target_os = "none")]
mod syscall;
pub mod utils;

#[cfg(target_os = "none")]
pub fn init() {
    display::init();
    println!("Initializing LuixOS kernel...");

    unsafe { asm!("cli") };
    memory::init();
    acpi::init();
    arch::apic::init();
    arch::gdt::init();
    arch::interrupt::init();

    unsafe {
        asm!("sti");
    };

    drivers::init();
}
#[panic_handler]
#[cfg(all(not(test), target_os = "none"))]
fn panic_handler(_info: &PanicInfo) -> ! {
    println!("Panic!, {}", _info);
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}

#[cfg(target_os = "none")]
pub mod test;
#[cfg(target_os = "none")]
pub fn test_runner(tests: &[&dyn test::Testable]) {
    use test::{exit_qemu, QemuExitCode};
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    print!("\n");
    print!("\x1b[92mAll tests passed! \x1b[0m\n");
    exit_qemu(QemuExitCode::Success);
}

#[cfg(all(test, target_os = "none"))]
#[no_mangle]
pub extern "C" fn _start() {
    init();
    test_main();
}

#[cfg(all(test, target_os = "none"))]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    use crate::test::{exit_qemu, QemuExitCode};
    println!("Panic!, {}", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
