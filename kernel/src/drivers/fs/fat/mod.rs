use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

use crate::drivers::fs::fat::boot_sector::Fat32BootSector;
use crate::drivers::fs::fat::directory::DirectoryEntry;
use crate::drivers::fs::gpt::GptPartitionEntry;
use crate::drivers::fs::path::Path;
use crate::drivers::fs::vfs::{Error, FileSystem, Inode, VirtualNode};
use crate::drivers::BlockDevice;

mod boot_sector;
mod directory;

const FAT32_EOC: u32 = 0x0FFFFFF8; // FAT32 end of cluster chain marker

pub(crate) struct Fat32FileSystem {
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
        // TODO: Check if the partition is FAT32
        let boot_sector = Fat32BootSector::read_from_disk(&partition, block_device.as_ref());
        let fat_area = FatArea::read_from_disk(&partition, &boot_sector, block_device.as_ref());
        Self {
            boot_sector,
            partition,
            fat_area,
            block_device: block_device.clone(),
        }
    }

    pub fn stat(&self, path: &Path) -> Option<DirectoryEntry> {
        self.find_entry(path)
    }

    fn find_entry(&self, path: &Path) -> Option<DirectoryEntry> {
        let mut found_entry = None;
        let mut cluster = self.boot_sector.root_cluster_number;
        let mut parts = path.iter().peekable();
        while let Some(part) = parts.next() {
            let mut walker =
                DirectoryWalker::new(cluster, &self.fat_area, self.block_device.clone());
            let entry = walker.find(|entry| {
                !entry.is_long_name() && entry.file_name().eq_ignore_ascii_case(part)
            })?;
            if parts.peek().is_some() && !entry.is_directory() {
                return None;
            }
            cluster = entry.cluster();
            found_entry = Some(entry);
        }
        found_entry
    }
}

impl FileSystem for Fat32FileSystem {
    fn open(&self, path: &Path<'_>) -> Result<VirtualNode, Error> {
        if path.iter().next().is_none() {
            return Err(Error::IsDirectory);
        }
        let entry = self.find_entry(path).ok_or(Error::NotFound)?;
        if entry.is_directory() {
            return Err(Error::IsDirectory);
        }
        Ok(VirtualNode {
            inode: Inode::new(entry.cluster().into()),
            size: entry.size as usize,
        })
    }

    fn read_at(&self, node: &VirtualNode, offset: usize, data: &mut [u8]) -> Result<usize, Error> {
        let count = min(data.len(), node.size.saturating_sub(offset));
        if count == 0 {
            return Ok(0);
        }

        let bytes_per_cluster = self.fat_area.bytes_per_cluster();
        if bytes_per_cluster == 0 {
            return Err(Error::Io);
        }
        let mut cluster_offset = offset % bytes_per_cluster;
        let mut copied = 0;
        let mut buffer = Vec::new();
        let clusters = self.fat_area.cluster_chain_iter(node.inode.as_u64() as u32);
        for cluster in clusters.skip(offset / bytes_per_cluster) {
            if !self
                .fat_area
                .read_cluster(cluster, &mut buffer, self.block_device.as_ref())
            {
                return Err(Error::Io);
            }
            let len = min(count - copied, bytes_per_cluster - cluster_offset);
            data[copied..copied + len]
                .copy_from_slice(&buffer[cluster_offset..cluster_offset + len]);
            copied += len;
            if copied == count {
                return Ok(copied);
            }
            cluster_offset = 0;
        }
        Err(Error::Io)
    }
}

#[derive(Debug)]
struct FatArea {
    start_sector: usize,
    sector_size: usize,
    sectors_per_cluster: usize,
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
            sectors_per_cluster: boot_sector.bios_parameter_block.sectors_per_cluster as usize,
            fat_start_sector: boot_sector.bios_parameter_block.reserved_sectors() as usize,
            sectors_per_fat: boot_sector.sectors_per_fat() as usize,
            first_data_sector: boot_sector.first_data_sector(),
            fat: Vec::new(),
        };
        fat_area.read_fat(block_device);
        fat_area
    }

    fn bytes_per_cluster(&self) -> usize {
        self.sector_size * self.sectors_per_cluster
    }

    /// Absolute device sector where `cluster` starts. Data clusters are numbered from 2.
    fn cluster_start_sector(&self, cluster: u32) -> usize {
        self.start_sector
            + self.first_data_sector
            + (cluster as usize - 2) * self.sectors_per_cluster
    }

    /// Reads every sector of `cluster` into `buffer`, replacing its contents.
    /// Returns false on an I/O error or an invalid cluster number.
    fn read_cluster(
        &self,
        cluster: u32,
        buffer: &mut Vec<u8>,
        block_device: &dyn BlockDevice,
    ) -> bool {
        let bytes_per_cluster = self.bytes_per_cluster();
        buffer.clear();
        if cluster < 2 || bytes_per_cluster == 0 {
            return false;
        }
        buffer.reserve(bytes_per_cluster);

        // One block read per sector: the block device does not yet handle
        // transfers that cross a page boundary.
        let first_sector = self.cluster_start_sector(cluster);
        let spare = &mut buffer.spare_capacity_mut()[..bytes_per_cluster];
        for (i, sector) in spare.chunks_exact_mut(self.sector_size).enumerate() {
            if block_device.read_block(first_sector + i, sector) != self.sector_size {
                return false;
            }
        }
        // SAFETY: every byte in 0..bytes_per_cluster was written by read_block above.
        unsafe { buffer.set_len(bytes_per_cluster) };
        true
    }

    pub fn cluster_chain_iter(&self, cluster: u32) -> FatClusterIterator<'_> {
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
        if self.current_cluster < 2 || self.current_cluster >= FAT32_EOC {
            return None;
        }

        let next_cluster = self.current_cluster;
        let fat_offset = self.current_cluster as usize * 4;

        let entry = self.fat.get(fat_offset..fat_offset + 4)?;
        self.current_cluster = u32::from_le_bytes(entry.try_into().unwrap()) & 0x0FFF_FFFF;

        Some(next_cluster)
    }
}

struct DirectoryWalker<'a> {
    cluster_data: Vec<u8>,
    cluster_offset: usize,
    fat_area: &'a FatArea,
    cluster_iter: FatClusterIterator<'a>,
    block_device: Arc<dyn BlockDevice>,
}

impl DirectoryWalker<'_> {
    pub fn new(
        cluster: u32,
        fat_area: &FatArea,
        block_device: Arc<dyn BlockDevice>,
    ) -> DirectoryWalker<'_> {
        let cluster_iter = fat_area.cluster_chain_iter(cluster);
        DirectoryWalker {
            cluster_data: Vec::new(),
            cluster_offset: 0,
            fat_area,
            cluster_iter,
            block_device,
        }
    }
}

impl Iterator for DirectoryWalker<'_> {
    type Item = DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cluster_offset + 32 > self.cluster_data.len() {
            let cluster = self.cluster_iter.next()?;
            if !self.fat_area.read_cluster(
                cluster,
                &mut self.cluster_data,
                self.block_device.as_ref(),
            ) {
                return None;
            }
            self.cluster_offset = 0;
        }

        let dir_entry = DirectoryEntry::from_sector(&self.cluster_data, self.cluster_offset);
        self.cluster_offset += 32;
        dir_entry
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use core::mem::MaybeUninit;

    use crate::drivers::fs::gpt::GuidedPartitionTable;
    use crate::drivers::nvme::NVME_CONTROLLERS;

    use super::*;

    const RAM_SECTOR: usize = 512;
    const RAM_SECTORS_PER_CLUSTER: usize = 4;
    const RAM_CLUSTER: usize = RAM_SECTOR * RAM_SECTORS_PER_CLUSTER;
    // One reserved sector and one FAT sector, so cluster 2 starts at sector 2.
    const RAM_FIRST_DATA_SECTOR: usize = 2;
    // Entries per sector; the test file's entry sits in the root cluster's second sector.
    const RAM_FILLER_ENTRIES: usize = RAM_SECTOR / 32;
    const RAM_FILE_SIZE: usize = RAM_CLUSTER + 100;

    struct RamDisk(Vec<u8>);

    impl BlockDevice for RamDisk {
        fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> usize {
            let start = sector * RAM_SECTOR;
            let Some(source) = self.0.get(start..start + buffer.len()) else {
                return 0;
            };
            for (dest, byte) in buffer.iter_mut().zip(source) {
                dest.write(*byte);
            }
            buffer.len()
        }

        fn write_block(&self, _sector: usize, _buffer: &[u8]) -> usize {
            0
        }
    }

    fn ram_cluster_offset(cluster: usize) -> usize {
        (RAM_FIRST_DATA_SECTOR + (cluster - 2) * RAM_SECTORS_PER_CLUSTER) * RAM_SECTOR
    }

    fn ram_file_byte(offset: usize) -> u8 {
        (offset % 251) as u8
    }

    fn write_dir_entry(disk: &mut [u8], offset: usize, name: &[u8; 11], cluster: u16, size: u32) {
        let entry = &mut disk[offset..offset + 32];
        entry[0..11].copy_from_slice(name);
        entry[11] = 0x20;
        entry[26..28].copy_from_slice(&cluster.to_le_bytes());
        entry[28..32].copy_from_slice(&size.to_le_bytes());
    }

    /// A FAT32 volume with four sectors per cluster:
    /// cluster 2 is the root directory, clusters 3 -> 4 hold DATA.BIN.
    fn ram_fat32_with_multi_sector_clusters() -> Fat32FileSystem {
        let mut disk = vec![0u8; ram_cluster_offset(5)];

        disk[11..13].copy_from_slice(&(RAM_SECTOR as u16).to_le_bytes());
        disk[13] = RAM_SECTORS_PER_CLUSTER as u8;
        disk[14..16].copy_from_slice(&1u16.to_le_bytes()); // reserved sectors
        disk[16] = 1; // FAT count
        disk[36..40].copy_from_slice(&1u32.to_le_bytes()); // sectors per FAT
        disk[44..48].copy_from_slice(&2u32.to_le_bytes()); // root cluster

        let fat = RAM_SECTOR;
        for (cluster, next) in [(2, 0x0FFF_FFFFu32), (3, 4), (4, 0x0FFF_FFFF)] {
            disk[fat + cluster * 4..fat + cluster * 4 + 4].copy_from_slice(&next.to_le_bytes());
        }

        let root = ram_cluster_offset(2);
        for i in 0..RAM_FILLER_ENTRIES {
            write_dir_entry(&mut disk, root + i * 32, b"FILL    TXT", 0, 0);
        }
        write_dir_entry(
            &mut disk,
            root + RAM_FILLER_ENTRIES * 32,
            b"DATA    BIN",
            3,
            RAM_FILE_SIZE as u32,
        );

        let data = ram_cluster_offset(3);
        for i in 0..RAM_FILE_SIZE {
            disk[data + i] = ram_file_byte(i);
        }

        // SAFETY: GptPartitionEntry is plain integers; all-zero is valid.
        let mut partition: GptPartitionEntry = unsafe { core::mem::zeroed() };
        partition.starting_lba = 0;
        Fat32FileSystem::read_from_disk(partition, Arc::new(RamDisk(disk)))
    }

    #[test]
    fn test_multi_sector_cluster_directory_walk() {
        let fs = ram_fat32_with_multi_sector_clusters();
        let entry = fs.stat(&Path::new("/DATA.BIN").unwrap()).unwrap();
        assert_eq!(entry.cluster(), 3);
        assert_eq!(entry.size as usize, RAM_FILE_SIZE);
    }

    #[test]
    fn test_multi_sector_cluster_read_at() {
        let fs = ram_fat32_with_multi_sector_clusters();
        let node = fs.open(&Path::new("/DATA.BIN").unwrap()).unwrap();

        let mut all = vec![0u8; RAM_FILE_SIZE];
        assert_eq!(fs.read_at(&node, 0, &mut all), Ok(RAM_FILE_SIZE));
        assert!(all
            .iter()
            .enumerate()
            .all(|(i, byte)| *byte == ram_file_byte(i)));

        // Within the first cluster, past its first sector, and across the cluster boundary.
        for offset in [600, RAM_CLUSTER - 8] {
            let mut chunk = [0u8; 16];
            assert_eq!(fs.read_at(&node, offset, &mut chunk), Ok(16));
            assert!(chunk
                .iter()
                .enumerate()
                .all(|(i, byte)| *byte == ram_file_byte(offset + i)));
        }

        let mut tail = [0u8; 200];
        assert_eq!(fs.read_at(&node, RAM_CLUSTER + 50, &mut tail), Ok(50));
        assert_eq!(fs.read_at(&node, RAM_FILE_SIZE, &mut tail), Ok(0));
    }

    #[test(target = "qemu")]
    fn test_fat_area_cluster_chain_iter() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller);

        let small_file = fs
            .stat(&Path::new("/test/deep/inside/deepfile.txt").unwrap())
            .unwrap();
        let mut iter = fs.fat_area.cluster_chain_iter(small_file.cluster());
        assert_eq!(iter.next(), Some(small_file.cluster()));
        assert_eq!(iter.next(), None);

        let kernel = fs.stat(&Path::new("/boot/kernel").unwrap()).unwrap();
        let bpb = fs.boot_sector.bios_parameter_block;
        let bytes_per_cluster =
            u32::from(bpb.bytes_per_sector()) * u32::from(bpb.sectors_per_cluster);
        let expected_clusters = kernel.size.div_ceil(bytes_per_cluster) as usize;
        assert!(expected_clusters > 1);
        let actual_clusters = fs
            .fat_area
            .cluster_chain_iter(kernel.cluster())
            .take(expected_clusters + 1)
            .count();
        assert_eq!(actual_clusters, expected_clusters);
    }

    #[test(target = "qemu")]
    fn test_find_file_on_root() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
        let entry = fs.find_entry(&Path::new("/README.md").unwrap());
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().file_name(), "README.MD");
    }

    #[test(target = "qemu")]
    fn test_stat() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
        let entry = fs.stat(&Path::new("/test/deep/inside/deepfile.txt").unwrap());
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.file_name(), "DEEPFILE.TXT");
        assert_eq!(entry.size, 0x22);
    }

    #[test(target = "qemu")]
    fn test_open() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
        let entry = fs
            .open(&Path::new("/test/deep/inside/deepfile.txt").unwrap())
            .unwrap();

        assert_ne!(entry.inode.as_u64(), 0);
        assert_eq!(entry.size, 0x22);
    }

    #[test(target = "qemu")]
    fn test_read() {
        let controller = NVME_CONTROLLERS.read()[0].clone();
        let gpt = GuidedPartitionTable::read_from_disk(controller.as_ref()).unwrap();
        let fs = Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
        let entry = fs
            .open(&Path::new("/test/deep/inside/deepfile.txt").unwrap())
            .unwrap();

        let buffer = &mut [0u8; 0x11];
        let bytes_read = fs.read_at(&entry, 0, buffer);
        assert_eq!(bytes_read, Ok(0x11));
        assert_eq!(buffer, b"This is a file in");

        let bytes_read = fs.read_at(&entry, 0x10, buffer);
        assert_eq!(bytes_read, Ok(0x11));
        assert_eq!(buffer, b"nside a long path");
    }

    #[test(target = "qemu")]
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

    #[test(target = "qemu")]
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
