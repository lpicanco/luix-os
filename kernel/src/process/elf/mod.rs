#[derive(Debug)]
#[repr(C)]
struct ElfHeader {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(u32)]
#[derive(Debug)]
pub enum ProgramHeaderType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    Phdr,
    Tls,
    Loos = 0x60000000,
    Hios = 0x6FFFFFFF,
    Loproc = 0x70000000,
    Hiproc = 0x7FFFFFFF,
}

#[repr(C)]
pub struct ProgramHeader {
    pub(crate) p_type: ProgramHeaderType,
    p_flags: u32,
    pub(crate) p_offset: u64,
    pub(crate) p_vaddr: u64,
    p_paddr: u64,
    pub(crate) p_filesz: u64,
    pub(crate) p_memsz: u64,
    p_align: u64,
}

impl ProgramHeader {
    pub(crate) fn contains_entry(&self, entry: u64) -> bool {
        const EXECUTABLE: u32 = 1;
        self.p_flags & EXECUTABLE != 0
            && entry >= self.p_vaddr
            && entry - self.p_vaddr < self.p_filesz
    }
}

#[repr(C)]
#[derive(Debug)]
struct SectionHeader {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

pub struct ElfFile<'a> {
    header: &'a ElfHeader,
    pub(crate) program_headers: &'a [ProgramHeader],
    section_headers: &'a [SectionHeader],
    buffer: &'a [u8],
}

impl<'a> ElfFile<'a> {
    pub(crate) fn parse(buffer: &'a [u8]) -> Self {
        let elf_header = unsafe { &*(buffer.as_ptr() as *const ElfHeader) };
        let program_headers = unsafe {
            core::slice::from_raw_parts(
                (buffer.as_ptr() as *const ProgramHeader).byte_add(elf_header.e_phoff as usize),
                elf_header.e_phnum as usize,
            )
        };
        let section_headers = unsafe {
            core::slice::from_raw_parts(
                (buffer.as_ptr() as *const SectionHeader).byte_add(elf_header.e_shoff as usize),
                elf_header.e_shnum as usize,
            )
        };
        Self {
            header: elf_header,
            program_headers,
            section_headers,
            buffer,
        }
    }

    pub(crate) fn entry_point(&self) -> u64 {
        self.header.e_entry
    }

    pub(crate) fn data(&self, header: &ProgramHeader) -> Option<&'a [u8]> {
        let end = header.p_offset.checked_add(header.p_filesz)?;
        self.buffer.get(header.p_offset as usize..end as usize)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use crate::drivers::fs::vfs;

    use super::*;

    #[test_case]
    fn test_load_elf() {
        let file = vfs::open("/boot/kernel").unwrap();
        let mut buffer = vec![0; file.len()];
        assert_eq!(file.read_at(0, &mut buffer), Ok(buffer.len()));

        let elf = ElfFile::parse(&buffer);

        assert_eq!(elf.header.e_ident[0..4], [0x7F, 0x45, 0x4C, 0x46]);
        assert_eq!(elf.header.e_phnum as usize, elf.program_headers.len());
        assert_eq!(elf.header.e_phentsize as usize, 0x38);
        assert_eq!(elf.header.e_shnum as usize, elf.section_headers.len());
        assert_eq!(elf.header.e_shentsize as usize, 0x40);
        assert_eq!(elf.program_headers.len(), 4);
        assert!(elf.entry_point() > 0xffffffff80000000 && elf.entry_point() < 0xffffffff90000000);
    }
}
