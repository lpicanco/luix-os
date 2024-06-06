use core::arch::asm;
use core::ptr;

use crate::allocate_frame;
use crate::arch::gdt::SELECTORS;
use crate::arch::memory::paging::{Page, PAGE_SIZE};
use crate::memory::address::VirtualAddress;
use crate::memory::MEMORY_MAPPER;
use crate::process::elf::{ElfFile, ProgramHeaderType};

const PROCESS_START: u64 = 0xF00D_C0DE_000;

pub fn spawn(elf_file: &ElfFile) {
    let code_size = 10 * PAGE_SIZE; // 10 pages. TODO: Calculate this properly
    let code_addr = PROCESS_START;

    let start_page = Page::containing_address(VirtualAddress::new(code_addr));
    let end_page = Page::containing_address(VirtualAddress::new(code_addr + code_size));

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = allocate_frame!();
        unsafe {
            MEMORY_MAPPER
                .get_unchecked()
                .map_page(page, frame, true, true)
        }
    }
    let page_ptr: *mut u8 = code_addr as *mut u8;
    let entry_point = elf_file.entry_point();

    for (header_idx, header) in elf_file
        .program_headers
        .iter()
        .filter(|h| matches!(h.p_type, ProgramHeaderType::Load))
        .enumerate()
    {
        let data = elf_file.data(header_idx);
        let addr = header.p_vaddr as usize;
        unsafe {
            ptr::copy(data.as_ptr(), page_ptr.offset(addr as isize), data.len());
        }
    }

    let selectors = SELECTORS.get().expect("GDT not initialized.");

    unsafe {
        asm!(
        "cli", // Disable interrupts
        "push rax",   // Stack segment (SS)
        "push rsi",   // Stack pointer (RSP)
        "push 0x200", // RFLAGS with interrupts enabled
        "push rdx",   // Code segment (CS)
        "push rdi",   // Instruction pointer (RIP)
        "iretq",
        in("rax") selectors.user_data.as_raw(),
        in("rsi") code_addr + code_size,
        in("rdx") selectors.user_code.as_raw(),
        in("rdi") code_addr + entry_point
        )
    }
}

mod tests {
    #[test_case]
    fn test_heap_allocator() {
        kernel_api::syscall::spawn("/boot/init");
    }
}
