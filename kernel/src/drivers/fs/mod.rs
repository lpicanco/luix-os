#[cfg(target_os = "none")]
use limine::request::KernelFileRequest;
#[cfg(target_os = "none")]
use uuid::Uuid;

#[cfg(target_os = "none")]
use crate::drivers::fs::fat::Fat32FileSystem;
#[cfg(target_os = "none")]
use crate::drivers::nvme::NVME_CONTROLLERS;
#[cfg(target_os = "none")]
use crate::trace;

mod fat;
mod gpt;
mod path;
pub(crate) mod vfs;

#[cfg(target_os = "none")]
static KERNEL_FILE_REQUEST: KernelFileRequest = KernelFileRequest::new();

#[cfg(target_os = "none")]
pub fn init() {
    // TODO: Replace the temporary FAT32 root mount with configurable mount points.
    vfs::mount_root(find_boot_fs().expect("Failed to find boot fs"));
}

#[cfg(target_os = "none")]
fn find_boot_fs() -> Result<Fat32FileSystem, vfs::Error> {
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

    let mut error = vfs::Error::NotFound;
    for controller in NVME_CONTROLLERS.read().iter() {
        match gpt::GuidedPartitionTable::read_from_disk(controller.as_ref()) {
            Ok(Some(gpt)) if gpt.disk_guid() == disk_id && gpt.partition_guid() == partition_id => {
                trace!("Found boot partition: {:#?}", gpt.partition_guid());
                return Fat32FileSystem::read_from_disk(gpt.entry, controller.clone());
            }
            Err(_) => error = vfs::Error::Io,
            Ok(_) => {}
        }
    }
    Err(error)
}
