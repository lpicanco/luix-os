use crate::drivers::fs::gpt::GptPartitionEntry;
use crate::drivers::{BlockDevice, BlockError};

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct BiosParameterBlock {
    boot_jump: [u8; 3],
    oem_identifier: [u8; 8],
    bytes_per_sector_le: u16,
    pub(super) sectors_per_cluster: u8,
    reserved_sectors_le: u16,
    fat_count: u8,
    root_directories_count_le: u16,
    total_sectors_in_logical_volume: u16,
    media_descriptor_type: u8,
    sectors_per_fat: u16,
    sectors_per_track: u16,
    head_count: u16,
    hidden_sector_count: u32,
    large_sector_count_le: u32,
}

impl BiosParameterBlock {
    pub fn bytes_per_sector(&self) -> u16 {
        u16::from_le(self.bytes_per_sector_le)
    }

    pub fn reserved_sectors(&self) -> u16 {
        u16::from_le(self.reserved_sectors_le)
    }

    fn root_directories_count(&self) -> u16 {
        u16::from_le(self.root_directories_count_le)
    }

    fn large_sector_count(&self) -> u32 {
        u32::from_le(self.large_sector_count_le)
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Fat32BootSector {
    pub bios_parameter_block: BiosParameterBlock,
    sectors_per_fat_le: u32,
    flags: u16,
    fat_version: u16,
    pub root_cluster_number: u32,
    fs_info_sector_number: u16,
    backup_boot_sector_number: u16,
    reserved: [u8; 12],
    drive_number: u8,
    windows_nt_flags: u8,
    signature: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    system_identifier: [u8; 8],
}

impl Fat32BootSector {
    pub fn read_from_disk(
        partition: &GptPartitionEntry,
        block_device: &dyn BlockDevice,
    ) -> Result<Self, BlockError> {
        let mut buffer = [0u8; 90];
        block_device.read_block(
            usize::try_from(partition.starting_lba).map_err(|_| BlockError)?,
            &mut buffer,
        )?;
        let sector = Self::parse(&buffer).ok_or(BlockError)?;
        let bpb = sector.bios_parameter_block;
        if bpb.bytes_per_sector() != 512
            || !bpb.sectors_per_cluster.is_power_of_two()
            || bpb.reserved_sectors() == 0
            || bpb.fat_count == 0
        {
            return Err(BlockError);
        }
        Ok(sector)
    }

    pub fn sectors_per_fat(&self) -> u32 {
        u32::from_le(self.sectors_per_fat_le)
    }

    fn root_dir_sectors(&self) -> u32 {
        ((self.bios_parameter_block.root_directories_count() * 32) as u32
            + (self.bios_parameter_block.bytes_per_sector() - 1) as u32)
            / self.bios_parameter_block.bytes_per_sector() as u32
    }

    /// First sector of the data region (cluster 2), relative to the partition start.
    pub fn first_data_sector(&self) -> usize {
        self.bios_parameter_block.reserved_sectors() as usize
            + (self.bios_parameter_block.fat_count as usize * self.sectors_per_fat() as usize)
            + self.root_dir_sectors() as usize
    }

    fn starting_fat_sector(&self) -> u32 {
        u32::from_le(self.bios_parameter_block.hidden_sector_count)
    }
}

#[cfg(all(test, target_os = "none"))]
mod tests {
    use crate::drivers::fs::boot_partition;

    use super::*;

    #[test(target = "qemu")]
    fn test_boot_sector() {
        let (partition, device) = boot_partition().unwrap();
        let bs = Fat32BootSector::read_from_disk(&partition, device.as_ref()).unwrap();

        assert!(matches!(bs.bios_parameter_block.boot_jump[0], 0xEB | 0xE9));

        assert_eq!(bs.bios_parameter_block.bytes_per_sector(), 512);
        assert_eq!(bs.bios_parameter_block.sectors_per_cluster, 1);
        assert_eq!(bs.bios_parameter_block.fat_count, 2);
        assert_eq!(bs.bios_parameter_block.root_directories_count(), 0);
        let partition_sectors = { partition.ending_lba } - { partition.starting_lba } + 1;
        assert_eq!(
            u64::from(bs.bios_parameter_block.large_sector_count()),
            partition_sectors
        );

        assert_eq!({ bs.root_cluster_number }, 2);
        assert_eq!(
            core::str::from_utf8(&bs.volume_label).unwrap(),
            "NO NAME    "
        );
        assert_eq!(
            core::str::from_utf8(&bs.system_identifier).unwrap(),
            "FAT32   "
        );

        assert_eq!(bs.root_dir_sectors(), 0);
        let data_sectors = partition_sectors
            .checked_sub(bs.first_data_sector() as u64)
            .unwrap();
        let fat_entries = u64::from(bs.sectors_per_fat())
            * u64::from(bs.bios_parameter_block.bytes_per_sector())
            / 4;
        assert!(data_sectors >= 65_525);
        assert!(fat_entries >= data_sectors + 2);
    }
}

impl BiosParameterBlock {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..36)?;
        Some(Self {
            boot_jump: bytes.get(0..3)?.try_into().ok()?,
            oem_identifier: bytes.get(3..11)?.try_into().ok()?,
            bytes_per_sector_le: u16::from_le_bytes(bytes.get(11..13)?.try_into().ok()?),
            sectors_per_cluster: *bytes.get(13)?,
            reserved_sectors_le: u16::from_le_bytes(bytes.get(14..16)?.try_into().ok()?),
            fat_count: *bytes.get(16)?,
            root_directories_count_le: u16::from_le_bytes(bytes.get(17..19)?.try_into().ok()?),
            total_sectors_in_logical_volume: u16::from_le_bytes(
                bytes.get(19..21)?.try_into().ok()?,
            ),
            media_descriptor_type: *bytes.get(21)?,
            sectors_per_fat: u16::from_le_bytes(bytes.get(22..24)?.try_into().ok()?),
            sectors_per_track: u16::from_le_bytes(bytes.get(24..26)?.try_into().ok()?),
            head_count: u16::from_le_bytes(bytes.get(26..28)?.try_into().ok()?),
            hidden_sector_count: u32::from_le_bytes(bytes.get(28..32)?.try_into().ok()?),
            large_sector_count_le: u32::from_le_bytes(bytes.get(32..36)?.try_into().ok()?),
        })
    }
}

impl Fat32BootSector {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..90)?;
        Some(Self {
            bios_parameter_block: BiosParameterBlock::parse(bytes.get(..36)?)?,
            sectors_per_fat_le: u32::from_le_bytes(bytes.get(36..40)?.try_into().ok()?),
            flags: u16::from_le_bytes(bytes.get(40..42)?.try_into().ok()?),
            fat_version: u16::from_le_bytes(bytes.get(42..44)?.try_into().ok()?),
            root_cluster_number: u32::from_le_bytes(bytes.get(44..48)?.try_into().ok()?),
            fs_info_sector_number: u16::from_le_bytes(bytes.get(48..50)?.try_into().ok()?),
            backup_boot_sector_number: u16::from_le_bytes(bytes.get(50..52)?.try_into().ok()?),
            reserved: bytes.get(52..64)?.try_into().ok()?,
            drive_number: *bytes.get(64)?,
            windows_nt_flags: *bytes.get(65)?,
            signature: *bytes.get(66)?,
            volume_id: u32::from_le_bytes(bytes.get(67..71)?.try_into().ok()?),
            volume_label: bytes.get(71..82)?.try_into().ok()?,
            system_identifier: bytes.get(82..90)?.try_into().ok()?,
        })
    }
}
