#![feature(abi_x86_interrupt)]
#![no_std]
mod arch;
mod display;
pub mod print;
mod serial;

pub fn init() {
    display::init();
    println!("Initializing Luix-OS kernel...");

    arch::interrupt::init();
}
