use limine::request::KernelFileRequest;
use spin::RwLock;
use uuid::Uuid;

use crate::drivers::fs::fat::Fat32FileSystem;
use crate::drivers::nvme::NVME_CONTROLLERS;
use crate::trace;

mod fat;
mod gpt;
mod path;

#[derive(PartialEq, Clone, Copy, Debug)]
pub struct Inode(u64);

impl Inode {
    fn new(inode: u64) -> Self {
        Self(inode)
    }

    fn as_u64(&self) -> u64 {
        self.0
    }
}

pub struct VirtualNode {
    inode: Inode,
    size: u64,
}

impl VirtualNode {
    pub fn new(inode: Inode, size: u64) -> Self {
        Self { inode, size }
    }
}

static KERNEL_FILE_REQUEST: KernelFileRequest = KernelFileRequest::new();

// TODO: Move to VFS
static BOOT_FS: RwLock<Option<Fat32FileSystem>> = RwLock::new(None);

pub fn init() {
    BOOT_FS
        .write()
        .replace(find_boot_fs().expect("Failed to find boot fs"));
}

fn find_boot_fs() -> Option<Fat32FileSystem> {
    let boot_partition = KERNEL_FILE_REQUEST
        .get_response()
        .expect("Failed to get boot partition");
    let disk_id: Uuid = boot_partition
        .file()
        .gpt_disk_id()
        .expect("Failed to get disk id")
        .into();
    let partition_id: Uuid = boot_partition
        .file()
        .gpt_partition_id()
        .expect("Failed to get partition id")
        .into();

    for controller in NVME_CONTROLLERS.read().iter() {
        let gpt = gpt::GuidedPartitionTable::read_from_disk(controller.as_ref());
        if let Some(gpt) = gpt {
            if gpt.disk_guid() == disk_id && gpt.partition_guid() == partition_id {
                trace!("Found boot partition: {:#?}", gpt.partition_guid());
                return Some(Fat32FileSystem::read_from_disk(
                    gpt.entry,
                    controller.clone(),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::drivers::fs::path::Path;
    use super::*;

    #[test_case]
    fn test_init_boot_fs() {
        let fs_opt = BOOT_FS.read();
        assert!(fs_opt.is_some());

        let fs = fs_opt.as_ref().unwrap();

        let entry = fs.open(&Path::new("/boot/kernel").unwrap());
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert!(entry.size > 3_000_000);
    }
}
