#[cfg(target_os = "none")]
use limine::request::KernelFileRequest;
#[cfg(target_os = "none")]
use uuid::Uuid;

#[cfg(target_os = "none")]
use crate::drivers::fs::fat::Fat32FileSystem;
#[cfg(target_os = "none")]
use crate::drivers::{BlockDevice, BLOCK_DEVICES};
#[cfg(target_os = "none")]
use crate::trace;
#[cfg(target_os = "none")]
use alloc::sync::Arc;

mod fat;
mod gpt;
mod path;
pub(crate) mod vfs;

#[cfg(target_os = "none")]
static KERNEL_FILE_REQUEST: KernelFileRequest = KernelFileRequest::new();

#[cfg(target_os = "none")]
pub fn init() {
    // TODO: Replace the temporary FAT32 root mount with configurable mount points.
    let (partition, device) = boot_partition().expect("Failed to find boot partition");
    vfs::mount_root(
        Fat32FileSystem::read_from_disk(partition, device).expect("Failed to mount boot fs"),
    );
}

#[cfg(target_os = "none")]
fn boot_partition() -> Result<(gpt::GptPartitionEntry, Arc<dyn BlockDevice>), vfs::Error> {
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
    for device in BLOCK_DEVICES.get().unwrap() {
        match gpt::GuidedPartitionTable::read_from_disk(device.as_ref()) {
            Ok(Some(gpt)) if gpt.disk_guid() == disk_id && gpt.partition_guid() == partition_id => {
                trace!("Found boot partition: {:#?}", gpt.partition_guid());
                return Ok((gpt.entry, device.clone()));
            }
            Err(_) => error = vfs::Error::Io,
            Ok(_) => {}
        }
    }
    Err(error)
}

#[cfg(all(test, target_os = "none"))]
pub(super) fn test_boot_device() -> Arc<dyn BlockDevice> {
    boot_partition().unwrap().1
}
