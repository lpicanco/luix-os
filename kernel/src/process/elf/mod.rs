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

    pub(crate) fn data(&self, segment_index: usize) -> &'a [u8] {
        let header = &self.program_headers[segment_index];
        &self.buffer[header.p_offset as usize..(header.p_offset + header.p_filesz) as usize]
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use crate::drivers::fs::path::Path;
    use crate::drivers::fs::BOOT_FS;

    use super::*;

    #[test_case]
    fn test_load_elf() {
        let fs_opt = BOOT_FS.read();
        let fs = fs_opt.as_ref().unwrap();
        let node = fs.open(&Path::new("/boot/kernel").unwrap()).unwrap();

        let mut buffer = vec![0; node.size as usize];
        fs.read(&node, 0, &mut buffer).unwrap();

        let elf = ElfFile::parse(&buffer);

        assert_eq!(elf.header.e_ident[0..4], [0x7F, 0x45, 0x4C, 0x46]);
        assert_eq!(elf.header.e_phnum as usize, elf.program_headers.len());
        assert_eq!(elf.header.e_phentsize as usize, 0x38);
        assert_eq!(elf.header.e_shnum as usize, elf.section_headers.len());
        assert_eq!(elf.header.e_shentsize as usize, 0x40);
        assert_eq!(elf.program_headers.len(), 4);
        assert_eq!(elf.section_headers.len(), 0x19);
        assert!(elf.entry_point() > 0xffffffff80000000 && elf.entry_point() < 0xffffffff90000000);
    }
}
