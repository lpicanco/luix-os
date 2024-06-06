#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::arch::asm;

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    kernel::init();
    kernel_api::syscall::spawn("/boot/init");

    loop {
        asm!("hlt");
    }
}
