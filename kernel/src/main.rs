#![no_std]
#![no_main]

use core::arch::asm;

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    print("Initializing Luix-OS kernel...\n");

    loop {
        asm!("hlt");
    }
}

fn print(string: &str) {
    for byte in string.bytes() {
        unsafe {
            let port = 0x3F8; // COM1
            asm!(
            "out dx, al",
            in("dx") port,
            in("al") byte,
            );
        }
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}
