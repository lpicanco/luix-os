#![no_std]
#![no_main]

use core::{hint, panic::PanicInfo};
use kernel_api::syscall::{exit, println, SPAWN};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Invalid arguments must return an error without faulting the kernel.
    for (address, length) in [
        (0usize, 1usize),
        (usize::MAX, 2),
        (0xffff_8000_0000_0000, 1),
        (0, 4097),
    ] {
        let status = unsafe { kernel_api::make_syscall!(SPAWN, address, length) };
        if status != 1 {
            stop("spawn-test: invalid user range accepted");
        }
    }
    let invalid_utf8 = [0xffu8];
    let status =
        unsafe { kernel_api::make_syscall!(SPAWN, invalid_utf8.as_ptr(), invalid_utf8.len()) };
    if status != 1 {
        stop("spawn-test: invalid UTF-8 accepted");
    }
    println("spawn-test: invalid user arguments rejected");
    let invalid = "/boot/MISSING.TST";
    let status: usize;
    // Restore DF before returning to Rust. The trap must clear it for kernel Rust as well.
    unsafe {
        core::arch::asm!("std", "int 0x80", "cld",
            inlateout("rax") SPAWN => status,
            in("rdi") invalid.as_ptr(), in("rsi") invalid.len());
    }
    if status != 1 {
        stop("spawn-test: trap with direction flag failed");
    }
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
