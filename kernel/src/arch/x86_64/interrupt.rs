use core::arch::asm;
use spin::Once;

use crate::arch::x86_64::idt;
use crate::arch::x86_64::idt::InterruptFrame;
use crate::println;

static IDT: Once<idt::InterruptDescriptorTable> = Once::new();

pub(crate) fn init() {
    IDT.call_once(|| {
        let mut idt = idt::InterruptDescriptorTable::new();
        idt.set_division_error_handler(divide_by_zero_handler);
        idt
    });
    IDT.get().unwrap().load();
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
