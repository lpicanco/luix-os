use limine::request::KernelFileRequest;
use uuid::Uuid;

use crate::drivers::fs::fat::Fat32FileSystem;
use crate::drivers::nvme::NVME_CONTROLLERS;
use crate::trace;

mod fat;
mod gpt;
mod path;
pub(crate) mod vfs;

static KERNEL_FILE_REQUEST: KernelFileRequest = KernelFileRequest::new();

pub fn init() {
    // TODO: Replace the temporary FAT32 root mount with configurable mount points.
    vfs::mount_root(find_boot_fs().expect("Failed to find boot fs"));
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
