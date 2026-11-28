#[cfg(target_os = "none")]
use kernel_core::boot;

#[cfg(target_os = "none")]
use crate::drivers::fs::fat::Fat32FileSystem;
#[cfg(target_os = "none")]
use crate::drivers::BlockDevice;
#[cfg(target_os = "none")]
use crate::trace;
#[cfg(target_os = "none")]
use alloc::sync::Arc;

mod fat;
mod gpt;
mod path;
pub(crate) mod vfs;

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
    let (disk_id, partition_id) = boot::partition_ids().ok_or(vfs::Error::NotFound)?;
    let mut error = vfs::Error::NotFound;
    for device in kernel_core::block::devices() {
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
