#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]
#![no_std]

extern crate alloc;

use core::arch::asm;

mod acpi;
mod arch;
mod display;
mod memory;
pub mod print;
mod serial;

pub fn init() {
    display::init();
    println!("Initializing LuixOS kernel...");

    unsafe { asm!("cli") };
    memory::init();
    acpi::init();
    arch::apic::init();
    arch::interrupt::init();

    unsafe {
        asm!("sti");
    };
}
