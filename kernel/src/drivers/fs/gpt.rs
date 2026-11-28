use crate::drivers::{BlockDevice, BlockError};

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
    pub ending_lba: u64,
    attributes: u64,
    partition_name: [u16; 36],
}

pub struct GuidedPartitionTable {
    pub header: GptHeader,
    pub entry: GptPartitionEntry,
}

impl GuidedPartitionTable {
    pub(crate) fn read_from_disk(
        block_device: &dyn BlockDevice,
    ) -> Result<Option<Self>, BlockError> {
        let mut buffer = [0u8; 92];
        block_device.read_block(1, &mut buffer)?;
        let header = GptHeader::parse(&buffer).ok_or(BlockError)?;
        if header.signature != *b"EFI PART" {
            return Ok(None);
        }

        // TODO: Check CRC32 and support more than one partition entry block
        let mut buffer = [0u8; 128];
        let sector = usize::try_from(header.partition_entry_lba).map_err(|_| BlockError)?;
        block_device.read_block(sector, &mut buffer)?;
        let entry = GptPartitionEntry::parse(&buffer).ok_or(BlockError)?;

        Ok(Some(Self { header, entry }))
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

#[cfg(all(test, target_os = "none"))]
mod tests {
    use alloc::string::{String, ToString};

    use crate::drivers::fs::test_boot_device;

    use super::*;

    #[test(target = "qemu")]
    fn test_read_from_disk() {
        let controller = test_boot_device();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref())
            .unwrap()
            .unwrap();

        assert_eq!(gpt.header.signature, *b"EFI PART");
        assert_eq!({ gpt.header.header_size }, 92);
        assert_eq!({ gpt.header.header_lba }, 1);
        assert_eq!({ gpt.header.partition_entry_lba }, 2);
        let entry_count = u64::from(gpt.header.partition_entry_count);
        let entry_size = u64::from(gpt.header.partition_entry_size);
        assert!(entry_count > 0);
        assert_eq!(entry_size, core::mem::size_of::<GptPartitionEntry>() as u64);
        let table_sectors = (entry_count * entry_size).div_ceil(512);
        assert!(gpt.header.partition_entry_lba + table_sectors <= gpt.header.first_usable_lba);
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
        assert_eq!({ gpt.entry.ending_lba }, { gpt.header.last_usable_lba });
    }
}

impl GptHeader {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..92)?;
        Some(Self {
            signature: bytes.get(0..8)?.try_into().ok()?,
            revision: u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?),
            header_size: u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?),
            header_crc32: u32::from_le_bytes(bytes.get(16..20)?.try_into().ok()?),
            reserved: u32::from_le_bytes(bytes.get(20..24)?.try_into().ok()?),
            header_lba: u64::from_le_bytes(bytes.get(24..32)?.try_into().ok()?),
            alternate_lba: u64::from_le_bytes(bytes.get(32..40)?.try_into().ok()?),
            first_usable_lba: u64::from_le_bytes(bytes.get(40..48)?.try_into().ok()?),
            last_usable_lba: u64::from_le_bytes(bytes.get(48..56)?.try_into().ok()?),
            disk_guid: bytes.get(56..72)?.try_into().ok()?,
            partition_entry_lba: u64::from_le_bytes(bytes.get(72..80)?.try_into().ok()?),
            partition_entry_count: u32::from_le_bytes(bytes.get(80..84)?.try_into().ok()?),
            partition_entry_size: u32::from_le_bytes(bytes.get(84..88)?.try_into().ok()?),
            partition_entry_crc32: u32::from_le_bytes(bytes.get(88..92)?.try_into().ok()?),
        })
    }
}

impl GptPartitionEntry {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..128)?;
        Some(Self {
            partition_type_guid: bytes.get(0..16)?.try_into().ok()?,
            unique_partition_guid: bytes.get(16..32)?.try_into().ok()?,
            starting_lba: u64::from_le_bytes(bytes.get(32..40)?.try_into().ok()?),
            ending_lba: u64::from_le_bytes(bytes.get(40..48)?.try_into().ok()?),
            attributes: u64::from_le_bytes(bytes.get(48..56)?.try_into().ok()?),
            partition_name: core::array::from_fn(|i| {
                u16::from_le_bytes([bytes[56 + i * 2], bytes[56 + i * 2 + 1]])
            }),
        })
    }
}
