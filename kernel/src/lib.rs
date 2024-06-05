#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![feature(assert_matches)]
#![feature(allocator_api)]
#![feature(new_uninit)]
#![feature(strict_provenance)]
#![feature(maybe_uninit_as_bytes)]
#![feature(naked_functions)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;

mod acpi;
mod arch;
mod display;
mod drivers;
mod memory;
pub mod print;
mod serial;
pub(crate) mod bits;
mod process;
mod syscall;

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
#[cfg(not(test))]
fn panic_handler(_info: &PanicInfo) -> ! {
    println!("Panic!, {}", _info);
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}

pub mod test;
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

#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() {
    init();
    test_main();
}

#[cfg(test)]
#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    use crate::test::{exit_qemu, QemuExitCode};
    println!("Panic!, {}", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}
