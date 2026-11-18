#![no_std]
#![no_main]

use core::{hint, panic::PanicInfo};
use kernel_api::syscall::{exit, println, SPAWN};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let child = "/boot/exit.tst";
    for _ in 0..2 {
        let status = unsafe { kernel_api::make_syscall!(SPAWN, child.as_ptr(), child.len()) };
        if status != 0 {
            stop("spawn-test: child launch failed");
        }
        println("spawn-test: resumed after child exit");
    }
    exit();
    stop("spawn-test: exit returned");
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    stop("spawn-test: panicked");
}

fn stop(message: &str) -> ! {
    println(message);
    loop {
        hint::spin_loop();
    }
}
