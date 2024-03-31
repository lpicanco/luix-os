#![no_std]
#![no_main]

use core::arch::asm;

use kernel::println;

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    kernel::init();

    loop {
        asm!("hlt");
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    println!("Panic!, {}", _info);
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}
