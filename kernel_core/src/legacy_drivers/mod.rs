//! Private hardware drivers scheduled for migration in LUIX-47.
pub(crate) use crate::block::{BlockDevice, BlockError, SECTOR_SIZE};
use alloc::{sync::Arc, vec::Vec};
mod nvme;
mod pci;
mod virtio_blk;

pub(crate) static BLOCK_DEVICES: spin::Once<Vec<Arc<dyn BlockDevice>>> = spin::Once::new();

pub(crate) fn init() {
    pci::init();
    BLOCK_DEVICES.call_once(|| {
        let mut devices = nvme::init();
        devices.extend(virtio_blk::init());
        devices
    });
}
