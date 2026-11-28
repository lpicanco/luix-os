use crate::process::elf::{ElfFile, ProgramHeaderType};
use kernel_core::user::{ProcessMemory, SyscallContext};

pub(crate) fn spawn(elf: &ElfFile<'_>, context: &mut SyscallContext<'_>) -> Result<(), ()> {
    let segments = || {
        elf.program_headers
            .iter()
            .filter(|h| h.p_type == ProgramHeaderType::Load)
    };
    let entry = elf.entry_point();
    if !segments().any(|h| h.contains_entry(entry))
        || segments().any(|h| {
            h.p_filesz > h.p_memsz
                || h.p_vaddr
                    .checked_add(h.p_memsz)
                    .is_none_or(|end| end > ProcessMemory::IMAGE_SIZE as u64)
                || elf.data(h).is_none()
        })
    {
        return Err(());
    }
    let mut memory = ProcessMemory::allocate().map_err(|_| ())?;
    for segment in segments() {
        memory
            .write(
                usize::try_from(segment.p_vaddr).map_err(|_| ())?,
                elf.data(segment).ok_or(())?,
            )
            .map_err(|_| ())?;
    }
    context.enter(memory, entry).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    #[test(target = "qemu")]
    fn test_spawn_exit_and_resume() {
        assert_eq!(kernel_core::user::launch("/boot/MISSING.TST"), 1);
        for _ in 0..2 {
            assert_eq!(kernel_core::user::launch("/boot/spawn.tst"), 0);
            assert_eq!(kernel_core::user::active_process_count(), 0);
        }
    }
}
