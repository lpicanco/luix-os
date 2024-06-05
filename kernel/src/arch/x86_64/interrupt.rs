use core::arch::asm;
use core::ptr;
use spin::Once;

use kernel_api::syscall::{EXIT, SPAWN};

use crate::arch::apic::io_apic::{Irq, IO_APIC};
use crate::arch::apic::local_apic::{end_of_interrupt, LOCAL_APIC};
use crate::arch::x86_64::idt::{InterruptFrame, Registers};
use crate::arch::x86_64::{idt, registers};
use crate::arch::PrivilegeLevel;
use crate::{println, syscall};

static IDT: Once<idt::InterruptDescriptorTable> = Once::new();

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptVector {
    Timer = 0x20,
    Keyboard,
    Syscall = 0x80,
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
        idt.set_handler(
            InterruptVector::Syscall as usize,
            syscall_handler_naked_wrap,
        );
        idt.set_privilege_level(InterruptVector::Syscall as usize, PrivilegeLevel::Ring3);
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

#[naked]
pub extern "x86-interrupt" fn syscall_handler_naked_wrap(interrupt_frame: InterruptFrame) {
    unsafe {
        asm!(
        "
                push rbp
                push rax
                push rbx
                push rcx
                push rdx
                push rsi
                push rdi
                push r8
                push r9
                push r10
                push r11
                push r12
                push r13
                push r14
                push r15
                mov rsi, rsp // registers
                mov rdi, rsp
                add rdi, 15*8 // interrupt_frame
                call {}
                pop r15
                pop r14
                pop r13
                pop r12
                pop r11
                pop r10
                pop r9
                pop r8
                pop rdi
                pop rsi
                pop rdx
                pop rcx
                pop rbx
                pop rax
                pop rbp
                iretq
            ",
        sym syscall_handler,
        options(noreturn)
        );
    }
}

// TODO: Move to the process
static mut STACK_FRAME: Option<InterruptFrame> = None;
static mut REGISTERS: Option<Registers> = None;

pub extern "C" fn syscall_handler(interrupt_frame: &mut InterruptFrame, regs: &mut Registers) {
    if regs.rax == SPAWN {
        unsafe {
            STACK_FRAME = Some(interrupt_frame.clone());
            REGISTERS = Some(regs.clone());
        }
    }

    syscall::dispatcher(regs.rax, regs.rdi, regs.rsi);

    if regs.rax == EXIT {
        unsafe {
            ptr::write_volatile(interrupt_frame as *mut InterruptFrame, STACK_FRAME.unwrap());
            ptr::write_volatile(regs, REGISTERS.unwrap());
        }
    }
}
