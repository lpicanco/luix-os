use alloc::vec;

use kernel_api::syscall::{EXIT, PRINT_LINE, SPAWN};

use crate::arch::{InterruptFrame, Registers};
use crate::drivers::fs::path::Path;
use crate::drivers::fs::BOOT_FS;
use crate::process::elf::ElfFile;
use crate::{println, trace};

pub fn dispatcher(
    interrupt_frame: &mut InterruptFrame,
    regs: &mut Registers,
    syscall_number: usize,
    arg1: usize,
    arg2: usize,
) -> usize {
    match syscall_number {
        SPAWN => spawn(interrupt_frame, regs, arg1, arg2),
        EXIT => exit(interrupt_frame, regs),
        PRINT_LINE => {
            println(arg1, arg2);
            0
        }
        _ => {
            println!("Unknown syscall: {}", syscall_number);
            1
        }
    }
}

fn spawn(interrupt_frame: &mut InterruptFrame, regs: &mut Registers, p0: usize, p1: usize) -> usize {
    let path = {
        let slice = unsafe { core::slice::from_raw_parts(p0 as *const u8, p1) };
        core::str::from_utf8(slice)
    }
    .unwrap();
    trace!("Spawning process: {}", path);

    let buffer = {
        let fs_opt = BOOT_FS.read();
        let fs = fs_opt.as_ref().unwrap();
        let node = fs.open(&Path::new(path).unwrap()).unwrap();
        let mut buffer = vec![0; node.size as usize];
        fs.read(&node, 0, &mut buffer).unwrap();
        buffer
    };

    let elf = ElfFile::parse(&buffer);
    usize::from(crate::process::loader::spawn(&elf, interrupt_frame, regs).is_err())
}

fn exit(interrupt_frame: &mut InterruptFrame, regs: &mut Registers) -> usize {
    trace!("Exiting process...");
    usize::from(crate::process::loader::exit(interrupt_frame, regs).is_err())
}

fn println(p0: usize, p1: usize) {
    let s = {
        let slice = unsafe { core::slice::from_raw_parts(p0 as *const u8, p1) };
        core::str::from_utf8(slice)
    };
    println!("{}", s.unwrap());
}
