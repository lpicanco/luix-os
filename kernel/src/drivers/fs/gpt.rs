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
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

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
            { gpt.entry.partition_type_guid },
            [
                0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E,
                0xC9, 0x3B
            ]
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
