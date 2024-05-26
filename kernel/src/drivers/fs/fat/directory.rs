use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug)]
pub struct DirectoryEntry {
    pub name: String,
    pub ext: String,
    pub attributes: u8,
    pub reserved: u8,
    pub creation_time_tenths: u8,
    pub creation_time: u16,
    pub creation_date: u16,
    pub access_date: u16,
    pub cluster_high: u16,
    pub modification_time: u16,
    pub modification_date: u16,
    pub cluster_low: u16,
    pub size: u32,
}

impl DirectoryEntry {
    pub fn is_long_name(&self) -> bool {
        self.attributes == 0x0F
    }

    fn is_directory(&self) -> bool {
        self.attributes & 0x10 != 0
    }

    pub fn cluster(&self) -> u32 {
        ((self.cluster_high as u32) << 16) | self.cluster_low as u32
    }

    pub fn file_name(&self) -> String {
        if self.ext.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.name, self.ext)
        }
    }

    pub fn from_sector(sector: &Vec<u8>, offset: usize) -> Option<Self> {
        let buffer = &sector[offset..offset + 32];
        if buffer[0] == 0xE5 || buffer[0] == 0x00 {
            return None;
        }

        let entry = Self {
            name: format_string(&buffer[0..8]),
            ext: format_string(&buffer[8..11]),
            attributes: buffer[11],
            reserved: buffer[12],
            creation_time_tenths: buffer[13],
            creation_time: u16::from_le_bytes(buffer[14..16].try_into().unwrap()),
            creation_date: u16::from_le_bytes(buffer[16..18].try_into().unwrap()),
            access_date: u16::from_le_bytes(buffer[18..20].try_into().unwrap()),
            cluster_high: u16::from_le_bytes(buffer[20..22].try_into().unwrap()),
            modification_time: u16::from_le_bytes(buffer[22..24].try_into().unwrap()),
            modification_date: u16::from_le_bytes(buffer[24..26].try_into().unwrap()),
            cluster_low: u16::from_le_bytes(buffer[26..28].try_into().unwrap()),
            size: u32::from_le_bytes(buffer[28..32].try_into().unwrap()),
        };
        Some(entry)
    }
}

fn format_string(value: &[u8]) -> String {
    let mut name = String::new();
    for c in value.iter() {
        if *c == 0x20 || *c == 0x00 {
            break;
        }
        name.push(*c as char);
    }
    name
}
