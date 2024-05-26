use alloc::boxed::Box;

use crate::drivers::fs::gpt::GptPartitionEntry;
use crate::drivers::BlockDevice;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct BiosParameterBlock {
    boot_jump: [u8; 3],
    oem_identifier: [u8; 8],
    bytes_per_sector_le: u16,
    sectors_per_cluster: u8,
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
    pub fn read_from_disk(partition: &GptPartitionEntry, block_device: &dyn BlockDevice) -> Self {
        let mut buffer = Box::<Self>::new_uninit();
        block_device.read_block(partition.starting_lba as usize, buffer.as_bytes_mut());
        unsafe { *buffer.assume_init() }
    }

    pub fn sectors_per_fat(&self) -> u32 {
        u32::from_le(self.sectors_per_fat_le)
    }

    fn root_dir_sectors(&self) -> u32 {
        ((self.bios_parameter_block.root_directories_count() * 32) as u32
            + (self.bios_parameter_block.bytes_per_sector() - 1) as u32)
            / self.bios_parameter_block.bytes_per_sector() as u32
    }

    pub fn first_data_sector(&self) -> usize {
        self.bios_parameter_block.reserved_sectors() as usize
            + (self.bios_parameter_block.fat_count as usize * self.sectors_per_fat() as usize)
            + self.root_dir_sectors() as usize
            - self.root_cluster_number as usize
    }

    fn starting_fat_sector(&self) -> u32 {
        u32::from_le(self.bios_parameter_block.hidden_sector_count)
    }
}

#[cfg(test)]
mod tests {
    use crate::drivers::fs::gpt::GuidedPartitionTable;
    use crate::drivers::nvme::NVME_CONTROLLERS;

    use super::*;

    #[test_case]
    fn test_boot_sector() {
        let controller = &NVME_CONTROLLERS.read()[0];
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let bs = Fat32BootSector::read_from_disk(&gpt.entry, controller.as_ref());

        assert_eq!(bs.bios_parameter_block.boot_jump, [0xEB, 0x58, 0x90]);
        assert_eq!(bs.bios_parameter_block.oem_identifier, *b"MTOO4043");

        assert_eq!(bs.bios_parameter_block.bytes_per_sector(), 512);
        assert_eq!(bs.bios_parameter_block.sectors_per_cluster, 1);
        assert_eq!(bs.bios_parameter_block.fat_count, 2);
        assert_eq!(bs.bios_parameter_block.root_directories_count(), 0);
        assert_eq!({ bs.bios_parameter_block.root_directories_count() }, 0);
        assert_eq!({ bs.bios_parameter_block.large_sector_count() }, 0x14800);
        assert_eq!(bs.sectors_per_fat(), 0x286);

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
        assert_eq!(bs.first_data_sector(), 0x52A);
    }
}
