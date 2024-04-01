use core::arch::asm;

use lazy_static::lazy_static;

use crate::arch::x86_64::idt;
use crate::arch::x86_64::idt::InterruptFrame;
use crate::println;

lazy_static! {
    static ref IDT: idt::InterruptDescriptorTable = {
        let mut idt = idt::InterruptDescriptorTable::new();
        idt.set_division_error_handler(divide_by_zero_handler);
        idt
    };
}
pub(crate) fn init() {
    IDT.load();
    println!("Interrupt Descriptor Table initialized.");
}

extern "x86-interrupt" fn divide_by_zero_handler(interrupt_frame: InterruptFrame) {
    println!("Error: Divide by zero.\n{}", interrupt_frame);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
