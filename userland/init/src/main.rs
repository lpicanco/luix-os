#![no_std]
#![no_main]

use core::panic::PanicInfo;
use kernel_api::syscall::{exit, println};

#[no_mangle]
pub extern "C" fn _start() {
    println("Init process started.");
    exit();
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}
