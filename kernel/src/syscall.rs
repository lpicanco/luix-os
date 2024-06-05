use alloc::vec;

use kernel_api::syscall::EXIT;
use kernel_api::syscall::PRINT_LINE;
use kernel_api::syscall::SPAWN;

use crate::drivers::fs::path::Path;
use crate::drivers::fs::BOOT_FS;
use crate::process::elf::ElfFile;
use crate::{println, trace};

pub fn dispatcher(syscall_number: usize, arg1: usize, arg2: usize) {
    match syscall_number {
        SPAWN => spawn(arg1, arg2),
        EXIT => exit(),
        PRINT_LINE => println(arg1, arg2),
        _ => println!("Unknown syscall: {}", syscall_number),
    }
}

fn spawn(p0: usize, p1: usize) {
    let path = {
        let slice = unsafe { core::slice::from_raw_parts(p0 as *const u8, p1) };
        core::str::from_utf8(slice)
    }
    .unwrap();
    trace!("Spawning process: {}", path);

    let fs_opt = BOOT_FS.read();
    let fs = fs_opt.as_ref().unwrap();
    let node = fs.open(&Path::new(path).unwrap()).unwrap();

    let mut buffer = vec![0; node.size as usize];
    fs.read(&node, 0, &mut buffer).unwrap();

    let elf = ElfFile::parse(&buffer);
    crate::process::loader::spawn(&elf);
}

fn exit() {
    trace!("Exiting process...");
}

fn println(p0: usize, p1: usize) {
    let s = {
        let slice = unsafe { core::slice::from_raw_parts(p0 as *const u8, p1) };
        core::str::from_utf8(slice)
    };
    println!("{}", s.unwrap());
}
