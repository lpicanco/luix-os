use core::arch::asm;
use spin::Once;

use crate::arch::apic::io_apic::{Irq, IO_APIC};
use crate::arch::apic::local_apic::{end_of_interrupt, LOCAL_APIC};
use crate::arch::x86_64::{idt, registers};
use crate::arch::x86_64::idt::InterruptFrame;
use crate::println;

static IDT: Once<idt::InterruptDescriptorTable> = Once::new();

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum InterruptVector {
    Timer = 32,
    Keyboard,
}

pub(crate) fn init() {
    IO_APIC
        .get()
        .expect("IO APIC not initialized.")
        .redirect(Irq::Keyboard, InterruptVector::Keyboard as u32);

    LOCAL_APIC
        .get()
        .expect("Local APIC not initialized.")
        .enable_interrupt(InterruptVector::Timer as u8);

    IDT.call_once(|| {
        let mut idt = idt::InterruptDescriptorTable::new();
        idt.set_division_error_handler(divide_by_zero_handler);
        idt.set_double_fault_handler(double_fault_handler);
        idt.set_general_protection_fault_handler(general_protection_fault_handler);
        idt.set_overflow_handler(double_fault_handler);
        idt.set_page_fault_handler(page_fault_handler);

        idt.set_handler(InterruptVector::Timer as usize, timer_interrupt_handler);
        idt.set_handler(
            InterruptVector::Keyboard as usize,
            keyboard_interrupt_handler,
        );
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

extern "x86-interrupt" fn double_fault_handler(interrupt_frame: InterruptFrame) {
    println!("Error: Double fault.\n{}", interrupt_frame);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(interrupt_frame: InterruptFrame) {
    println!("Error: General protection fault.\n{}", interrupt_frame);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

extern "x86-interrupt" fn page_fault_handler(interrupt_frame: InterruptFrame) {
    println!("Error: Page fault at {}\n{}", registers::read_cr2(), interrupt_frame);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_: InterruptFrame) {
    end_of_interrupt();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_: InterruptFrame) {
    let code: u16;
    unsafe {
        asm!("in ax, dx", out("ax") code, in("dx") 0x60, options(nomem, nostack, preserves_flags));
    };

    println!("Keyboard interrupt: {:#X}\n", code);
    end_of_interrupt();
}
