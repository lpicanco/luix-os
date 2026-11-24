#[cfg(target_os = "none")]
use crate::println;
#[cfg(target_os = "none")]
use alloc::{sync::Arc, vec::Vec};
use core::mem::MaybeUninit;

pub(crate) mod fs;
#[cfg(target_os = "none")]
pub mod keyboard;
#[cfg(target_os = "none")]
pub(crate) mod nvme;
#[cfg(target_os = "none")]
mod pci;
#[cfg(target_os = "none")]
mod virtio_blk;

#[cfg(target_os = "none")]
static BLOCK_DEVICES: spin::Once<Vec<Arc<dyn BlockDevice>>> = spin::Once::new();

#[cfg(target_os = "none")]
pub fn init() {
    pci::init();
    BLOCK_DEVICES.call_once(|| {
        let mut devices = nvme::init();
        devices.extend(virtio_blk::init());
        devices
    });
    fs::init();
    println!("Drivers initialized.");
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BlockError;

pub const SECTOR_SIZE: usize = 512;

/// `read_block` accepts any buffer length and fills it from the start of `sector`.
/// `write_block` requires a multiple of `SECTOR_SIZE` and fails otherwise.
///
/// # Safety
/// Implementors must initialize every byte of the supplied buffer before returning
/// `Ok(())` from `read_block`. Filesystem callers rely on this to read typed data.
pub unsafe trait BlockDevice: Send + Sync {
    fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> Result<(), BlockError>;
    fn write_block(&self, sector: usize, buffer: &[u8]) -> Result<(), BlockError>;
    fn sector_count(&self) -> u64;
}

#[cfg(all(test, target_os = "none"))]
mod tests {
    use super::*;

    #[test(target = "qemu")]
    fn boot_disk_sector_count() {
        // The test image is 42 MiB.
        assert_eq!(fs::test_boot_device().sector_count(), 86_016);
    }

    #[test(target = "qemu")]
    fn failed_boot_disk_read_preserves_buffer_and_next_read_succeeds() {
        let device = fs::test_boot_device();
        let mut buffer = [MaybeUninit::new(0xa5); 512];
        let end = device.sector_count() as usize;
        assert_eq!(device.read_block(end, &mut buffer), Err(BlockError));
        assert!(buffer
            .iter()
            .all(|byte| unsafe { byte.assume_init() } == 0xa5));
        device.read_block(1, &mut buffer).unwrap();
        assert_eq!(&unsafe { buffer.assume_init_ref() }[..8], b"EFI PART");
    }

    #[test(target = "qemu")]
    fn boot_disk_write_round_trip() {
        let device = fs::test_boot_device();
        // The last two sectors before the test image's first partition are unused.
        let mut saved = [MaybeUninit::uninit(); 1024];
        device.read_block(2046, &mut saved).unwrap();
        let expected = [0x5a; 1024];
        device.write_block(2046, &expected).unwrap();
        assert_eq!(device.write_block(2046, &[0xab; 513]), Err(BlockError));
        let mut result = [MaybeUninit::uninit(); 1024];
        device.read_block(2046, &mut result).unwrap();
        device
            .write_block(2046, unsafe { saved.assume_init_ref() })
            .unwrap();
        assert_eq!(unsafe { result.assume_init_ref() }, &expected);
    }
}
