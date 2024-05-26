use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::fs::fat::boot_sector::Fat32BootSector;
use crate::drivers::fs::fat::directory::DirectoryEntry;
use crate::drivers::fs::gpt::GptPartitionEntry;
use crate::drivers::fs::path::Path;
use crate::drivers::BlockDevice;

mod boot_sector;
mod directory;

const FAT32_EOC: u32 = 0x0FFFFFF8; // FAT32 end of cluster chain marker

pub struct Fat32FileSystem {
    boot_sector: Fat32BootSector,
    partition: GptPartitionEntry,
    fat_area: FatArea,
    block_device: Arc<dyn BlockDevice>,
}

impl Fat32FileSystem {
    pub fn read_from_disk(
        partition: GptPartitionEntry,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        let boot_sector = Fat32BootSector::read_from_disk(&partition, block_device.as_ref());
        let fat_area = FatArea::read_from_disk(&partition, &boot_sector, block_device.as_ref());
        Self {
            boot_sector,
            partition,
            fat_area,
            block_device: block_device.clone(),
        }
    }

    fn find_entry(&self, path: &Path) -> Option<DirectoryEntry> {
        let mut found_entry = None;
        let mut cluster = self.boot_sector.root_cluster_number;
        for path_part in path.iter() {
            let mut walker =
                DirectoryWalker::new(cluster, &self.fat_area, self.block_device.clone());

            found_entry =
                match walker.find(|entry| entry.file_name().eq_ignore_ascii_case(path_part)) {
                    Some(entry) => {
                        cluster = entry.cluster();
                        Some(entry)
                    }
                    None => return None,
                };
        }

        found_entry
    }
}

#[derive(Debug)]
struct FatArea {
    start_sector: usize,
    sector_size: usize,
    fat_start_sector: usize,
    sectors_per_fat: usize,
    first_data_sector: usize,
    fat: Vec<u8>,
}

impl FatArea {
    pub fn read_from_disk(
        partition: &GptPartitionEntry,
        boot_sector: &Fat32BootSector,
        block_device: &dyn BlockDevice,
    ) -> Self {
        let mut fat_area = FatArea {
            start_sector: partition.starting_lba as usize,
            sector_size: boot_sector.bios_parameter_block.bytes_per_sector() as usize,
            fat_start_sector: boot_sector.bios_parameter_block.reserved_sectors() as usize,
            sectors_per_fat: boot_sector.sectors_per_fat() as usize,
            first_data_sector: boot_sector.first_data_sector(),
            fat: Vec::new(),
        };
        fat_area.read_fat(block_device);
        fat_area
    }

    pub fn cluster_chain_iter<'a>(&self, cluster: u32) -> FatClusterIterator {
        FatClusterIterator {
            current_cluster: cluster,
            fat: &self.fat,
        }
    }

    fn read_fat(&mut self, block_device: &dyn BlockDevice) {
        for i in 0..self.sectors_per_fat as u32 {
            self.fat.extend(self.read_fat_sector(i, block_device))
        }
    }

    fn read_fat_sector(&self, fat_index: u32, block_device: &dyn BlockDevice) -> Vec<u8> {
        let sector = self.start_sector + self.fat_start_sector + fat_index as usize;
        self.read_sector(sector, block_device)
    }

    fn read_sector(&self, sector: usize, block_device: &dyn BlockDevice) -> Vec<u8> {
        let mut buffer = Box::<[u8; 512]>::new_uninit();
        block_device.read_block(sector, buffer.as_bytes_mut());
        unsafe { *buffer.assume_init() }.to_vec()
    }
}

struct FatClusterIterator<'a> {
    current_cluster: u32,
    fat: &'a Vec<u8>,
}

impl<'a> Iterator for FatClusterIterator<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_cluster >= FAT32_EOC {
            return None;
        }

        let next_cluster = self.current_cluster;
        let fat_offset = self.current_cluster as usize * 4;

        self.current_cluster = u32::from_le_bytes([
            self.fat[fat_offset],
            self.fat[fat_offset + 1],
            self.fat[fat_offset + 2],
            self.fat[fat_offset + 3],
        ]) & 0x0FFF_FFFF; // Mask to 28 bits to ignore the high 4 bits

        Some(next_cluster)
    }
}

struct DirectoryWalker<'a> {
    sector: Vec<u8>,
    sector_offset: usize,
    fat_area: &'a FatArea,
    cluster_iter: FatClusterIterator<'a>,
    block_device: Arc<dyn BlockDevice>,
}

impl DirectoryWalker<'_> {
    pub fn new<'a>(
        cluster: u32,
        fat_area: &'a FatArea,
        block_device: Arc<dyn BlockDevice>,
    ) -> DirectoryWalker<'a> {
        let cluster_iter = fat_area.cluster_chain_iter(cluster);
        DirectoryWalker {
            sector: Vec::new(),
            sector_offset: fat_area.sector_size,
            fat_area,
            cluster_iter,
            block_device,
        }
    }

    fn read_sector(&self, sector: usize) -> Vec<u8> {
        let sector = self.fat_area.start_sector + sector + self.fat_area.first_data_sector;
        let mut buffer = Box::<[u8; 512]>::new_uninit();
        self.block_device.read_block(sector, buffer.as_bytes_mut());
        unsafe { *buffer.assume_init() }.to_vec()
    }
}

impl Iterator for DirectoryWalker<'_> {
    type Item = DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sector_offset >= self.fat_area.sector_size {
            match self.cluster_iter.next() {
                Some(cluster) => {
                    self.sector = self.read_sector(cluster as usize);
                    self.sector_offset = 0;
                }
                None => {
                    return None;
                }
            }
        }

        let dir_entry = DirectoryEntry::from_sector(&self.sector, self.sector_offset);
        self.sector_offset += 32;
        dir_entry
    }
}

#[cfg(test)]
mod tests {
    use crate::drivers::fs::gpt::GuidedPartitionTable;
    use crate::drivers::nvme::NVME_CONTROLLERS;

    use super::*;

    #[test_case]
    fn test_fat_area_cluster_chain_iter() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller);

        let mut iter = fs.fat_area.cluster_chain_iter(2);
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);

        let mut iter = fs.fat_area.cluster_chain_iter(6);
        assert_eq!(iter.next(), Some(6));
        assert_eq!(iter.next(), None);

        let mut iter = fs.fat_area.cluster_chain_iter(7);
        assert_eq!(iter.next(), Some(7));
        assert_eq!(iter.next(), Some(8));
        assert_eq!(iter.next(), Some(9));
        assert!(iter.count() > 5000)
    }

    #[test_case]
    fn test_find_file_on_root() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
        let entry = fs.find_entry(&Path::new("/README.md").unwrap());
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().file_name(), "README.MD");
    }

    #[test_case]
    fn test_find_file_deep_inside() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller);

        let entry = fs.find_entry(&Path::new("/test/deep/inside/deepfile.txt").unwrap());
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.file_name(), "DEEPFILE.TXT");
        assert_eq!(entry.size, 0x22);
    }

    #[test_case]
    fn test_directory_walker() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());

        let mut walker = DirectoryWalker::new(2, &fs.fat_area, controller.clone())
            .filter(|entry| !entry.is_long_name());

        assert_eq!(walker.next().unwrap().name, "EFI");
        assert_eq!(walker.next().unwrap().name, "BOOT");
        let test_dir = walker.next().unwrap();
        assert_eq!(test_dir.name, "TEST");

        assert_eq!(walker.next().unwrap().name, "LONG-D~1");
        assert_eq!(walker.next().unwrap().name, "README");

        let mut walker = DirectoryWalker::new(test_dir.cluster(), &fs.fat_area, controller)
            .filter(|entry| !entry.is_long_name());

        assert_eq!(walker.next().unwrap().name, ".");
        assert_eq!(walker.next().unwrap().name, "..");
        let test_dir = walker.next().unwrap();
        assert_eq!(test_dir.name, "DEEP");
    }
}
