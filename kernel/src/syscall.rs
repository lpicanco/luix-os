use crate::drivers::fs::vfs;
use crate::process::elf::ElfFile;
use alloc::vec;
use kernel_api::syscall::{EXIT, PRINT_LINE, SPAWN};
use kernel_core::user::SyscallContext;

pub fn dispatcher(context: &mut SyscallContext<'_>) -> usize {
    match context.number() {
        EXIT => usize::from(context.exit().is_err()),
        SPAWN | PRINT_LINE => {
            let Ok(bytes) = context.copy_argument(context.arg(0), context.arg(1)) else {
                return 1;
            };
            let Ok(text) = core::str::from_utf8(&bytes) else {
                return 1;
            };
            if context.number() == PRINT_LINE {
                crate::println!("{text}");
                return 0;
            }
            let Ok(file) = vfs::open(text) else {
                return 1;
            };
            let mut buffer = vec![0; file.len()];
            if file.read_at(0, &mut buffer) != Ok(buffer.len()) {
                return 1;
            }
            let Ok(elf) = ElfFile::parse(&buffer) else {
                return 1;
            };
            usize::from(crate::process::loader::spawn(&elf, context).is_err())
        }
        _ => 1,
    }
}
