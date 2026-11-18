#![no_std]
#![no_main]

use core::{hint, panic::PanicInfo};
use kernel_api::syscall::{exit, println};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println("exit-test: exiting");
    exit();
    stop("exit-test: exit returned");
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop("exit-test: panicked");
}

fn stop(message: &str) -> ! {
    println(message);
    loop {
        hint::spin_loop();
    }
}
