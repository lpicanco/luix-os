#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]
#![no_std]

use core::arch::asm;

mod acpi;
mod arch;
mod display;
pub mod print;
mod serial;

pub fn init() {
    display::init();
    println!("Initializing Luix-OS kernel...");

    unsafe { asm!("cli") };
    acpi::init();
    arch::interrupt::init();

    unsafe {
        asm!("sti");
    };
}
