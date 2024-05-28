use alloc::boxed::Box;

use crate::drivers::BlockDevice;

#[repr(C, packed)]
pub struct GptHeader {
    signature: [u8; 8],
    revision: u32,
    header_size: u32,
    header_crc32: u32,
    reserved: u32,
    header_lba: u64,
    alternate_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    disk_guid: [u8; 16],
    partition_entry_lba: u64,
    partition_entry_count: u32,
    partition_entry_size: u32,
    partition_entry_crc32: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GptPartitionEntry {
    partition_type_guid: [u8; 16],
    unique_partition_guid: [u8; 16],
    pub starting_lba: u64,
    ending_lba: u64,
    attributes: u64,
    partition_name: [u16; 36],
}

pub struct GuidedPartitionTable {
    pub header: GptHeader,
    pub entry: GptPartitionEntry,
}

impl GuidedPartitionTable {
    pub(crate) fn read_from_disk(block_device: &dyn BlockDevice) -> Option<Self> {
        let mut buffer = Box::<GptHeader>::new_uninit();
        block_device.read_block(1, buffer.as_bytes_mut());

        let header = unsafe { *buffer.assume_init() };
        if header.signature != *b"EFI PART" {
            return None;
        }

        // TODO: Check CRC32 and support more than one partition entry block
        let mut buffer = Box::<GptPartitionEntry>::new_uninit();
        block_device.read_block(header.partition_entry_lba as usize, buffer.as_bytes_mut());
        let entry = unsafe { *buffer.assume_init() };

        Some(Self { header, entry })
    }

    pub fn disk_guid(&self) -> uuid::Uuid {
        self.parse_guid(&self.header.disk_guid)
    }

    pub fn partition_guid(&self) -> uuid::Uuid {
        self.parse_guid(&self.entry.unique_partition_guid)
    }

    pub fn partition_type_guid(&self) -> uuid::Uuid {
        self.parse_guid(&self.entry.partition_type_guid)
    }

    fn parse_guid(&self, bytes: &[u8; 16]) -> uuid::Uuid {
        uuid::Uuid::from_fields(
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            <&[u8; 8]>::try_from(&bytes[8..16]).unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};

    use crate::drivers::nvme::NVME_CONTROLLERS;

    use super::*;

    #[test_case]
    fn test_read_from_disk() {
        let controller = &NVME_CONTROLLERS.read()[0];
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();

        assert_eq!(gpt.header.signature, *b"EFI PART");
        assert_eq!({ gpt.header.header_size }, 92);
        assert_eq!({ gpt.header.header_lba }, 1);
        assert_eq!({ gpt.header.partition_entry_lba }, 2);
        assert_eq!({ gpt.header.partition_entry_count }, 56);
        assert_eq!(
            gpt.partition_type_guid().to_string(),
            "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"
        );
        let partition_name = gpt.entry.partition_name;
        assert_eq!(
            String::from_utf16_lossy(&partition_name).trim_end_matches('\0'),
            "luix-os-test"
        );
        assert_eq!({ gpt.entry.starting_lba }, 0x800);
        assert_eq!({ gpt.entry.ending_lba }, 0x14FDE);
    }
}
