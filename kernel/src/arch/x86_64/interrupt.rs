use core::arch::asm;

use lazy_static::lazy_static;

use crate::arch::x86_64::idt;
use crate::println;

lazy_static! {
    static ref IDT: idt::InterruptDescriptorTable = {
        let mut idt = idt::InterruptDescriptorTable::new();
        idt.set_divide_by_zero_handler(divide_by_zero_handler);
        idt
    };
}
pub(crate) fn init() {
    IDT.load();
}

extern "x86-interrupt" fn divide_by_zero_handler() {
    println!("Error: Divide by zero.");

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
