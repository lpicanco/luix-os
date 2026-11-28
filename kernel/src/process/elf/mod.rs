//! Checked ELF64 decoding. No Rust references are formed from file bytes.
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfError;

struct ElfHeader {
    e_ident: [u8; 16],
    e_entry: u64,
    e_phnum: u16,
    e_phentsize: u16,
    e_shnum: u16,
    e_shentsize: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramHeaderType {
    Load,
    Other(u32),
}

pub struct ProgramHeader {
    pub(crate) p_type: ProgramHeaderType,
    p_flags: u32,
    pub(crate) p_offset: u64,
    pub(crate) p_vaddr: u64,
    pub(crate) p_filesz: u64,
    pub(crate) p_memsz: u64,
}

impl ProgramHeader {
    pub(crate) fn contains_entry(&self, entry: u64) -> bool {
        self.p_flags & 1 != 0 && entry >= self.p_vaddr && entry - self.p_vaddr < self.p_filesz
    }
}

pub struct ElfFile<'a> {
    header: ElfHeader,
    pub(crate) program_headers: Vec<ProgramHeader>,
    buffer: &'a [u8],
}

fn bytes<const N: usize>(data: &[u8], at: usize) -> Result<[u8; N], ElfError> {
    data.get(at..at.checked_add(N).ok_or(ElfError)?)
        .ok_or(ElfError)?
        .try_into()
        .map_err(|_| ElfError)
}
fn u16_at(data: &[u8], at: usize) -> Result<u16, ElfError> {
    Ok(u16::from_le_bytes(bytes(data, at)?))
}
fn u32_at(data: &[u8], at: usize) -> Result<u32, ElfError> {
    Ok(u32::from_le_bytes(bytes(data, at)?))
}
fn u64_at(data: &[u8], at: usize) -> Result<u64, ElfError> {
    Ok(u64::from_le_bytes(bytes(data, at)?))
}
fn table(data: &[u8], offset: u64, count: u16, size: u16) -> Result<&[u8], ElfError> {
    let start = usize::try_from(offset).map_err(|_| ElfError)?;
    let len = usize::from(count)
        .checked_mul(usize::from(size))
        .ok_or(ElfError)?;
    data.get(start..start.checked_add(len).ok_or(ElfError)?)
        .ok_or(ElfError)
}
impl<'a> ElfFile<'a> {
    pub(crate) fn parse(buffer: &'a [u8]) -> Result<Self, ElfError> {
        let ident: [u8; 16] = bytes(buffer, 0)?;
        if &ident[..4] != b"\x7fELF"
            || ident[4..7] != [2, 1, 1]
            || !matches!(u16_at(buffer, 16)?, 2 | 3)
            || u16_at(buffer, 18)? != 62
            || u32_at(buffer, 20)? != 1
            || u16_at(buffer, 52)? != 64
        {
            return Err(ElfError);
        }
        let header = ElfHeader {
            e_ident: ident,
            e_entry: u64_at(buffer, 24)?,
            e_phentsize: u16_at(buffer, 54)?,
            e_phnum: u16_at(buffer, 56)?,
            e_shentsize: u16_at(buffer, 58)?,
            e_shnum: u16_at(buffer, 60)?,
        };
        if header.e_phentsize != 56 || (header.e_shnum != 0 && header.e_shentsize != 64) {
            return Err(ElfError);
        }
        let ph = table(
            buffer,
            u64_at(buffer, 32)?,
            header.e_phnum,
            header.e_phentsize,
        )?;
        if header.e_shnum != 0 {
            table(
                buffer,
                u64_at(buffer, 40)?,
                header.e_shnum,
                header.e_shentsize,
            )?;
        }
        let mut program_headers = Vec::new();
        program_headers
            .try_reserve(usize::from(header.e_phnum))
            .map_err(|_| ElfError)?;
        for p in ph.chunks_exact(56) {
            let kind = u32_at(p, 0)?;
            let entry = ProgramHeader {
                p_type: if kind == 1 {
                    ProgramHeaderType::Load
                } else {
                    ProgramHeaderType::Other(kind)
                },
                p_flags: u32_at(p, 4)?,
                p_offset: u64_at(p, 8)?,
                p_vaddr: u64_at(p, 16)?,
                p_filesz: u64_at(p, 32)?,
                p_memsz: u64_at(p, 40)?,
            };
            if kind == 1 {
                if entry.p_filesz > entry.p_memsz
                    || entry.p_vaddr.checked_add(entry.p_memsz).is_none()
                {
                    return Err(ElfError);
                }
                let end = entry.p_offset.checked_add(entry.p_filesz).ok_or(ElfError)?;
                if end > buffer.len() as u64 {
                    return Err(ElfError);
                }
            }
            program_headers.push(entry);
        }
        Ok(Self {
            header,
            program_headers,
            buffer,
        })
    }
    pub(crate) fn entry_point(&self) -> u64 {
        self.header.e_entry
    }
    pub(crate) fn data(&self, header: &ProgramHeader) -> Option<&'a [u8]> {
        let start = usize::try_from(header.p_offset).ok()?;
        let size = usize::try_from(header.p_filesz).ok()?;
        self.buffer.get(start..start.checked_add(size)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_unaligned_and_overflowing_images() {
        for len in 0..64 {
            assert!(ElfFile::parse(&[0u8; 64][..len]).is_err());
        }
        let mut image = [0u8; 121];
        let b = &mut image[1..];
        b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        b[16..18].copy_from_slice(&2u16.to_le_bytes());
        b[18..20].copy_from_slice(&62u16.to_le_bytes());
        b[20..24].copy_from_slice(&1u32.to_le_bytes());
        b[32..40].copy_from_slice(&64u64.to_le_bytes());
        b[52..54].copy_from_slice(&64u16.to_le_bytes());
        b[54..56].copy_from_slice(&56u16.to_le_bytes());
        b[56..58].copy_from_slice(&1u16.to_le_bytes());
        assert!(ElfFile::parse(b).is_ok()); // unaligned storage is supported
        b[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(ElfFile::parse(b).is_err());
    }
    #[test(target = "qemu")]
    fn test_load_elf() {
        let file = crate::drivers::fs::vfs::open("/boot/kernel").unwrap();
        let mut data = alloc::vec![0u8; file.len()];
        assert_eq!(file.read_at(0, &mut data), Ok(data.len()));
        let elf = ElfFile::parse(&data).unwrap();
        assert_eq!(&elf.header.e_ident[..4], b"\x7fELF");
        assert_eq!(elf.program_headers.len(), usize::from(elf.header.e_phnum));
        assert!(elf.entry_point() >= 0xffffffff80000000);
    }
}
