pub(crate) use kernel_core::block::{BlockDevice, BlockError};
pub(crate) mod fs;
#[cfg(target_os = "none")]
pub mod keyboard;

#[cfg(target_os = "none")]
pub fn init() {
    fs::init();
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
        let mut buffer = [0xa5u8; 512];
        let end = device.sector_count() as usize;
        assert_eq!(device.read_block(end, &mut buffer), Err(BlockError));
        assert!(buffer.iter().all(|byte| *byte == 0xa5));
        device.read_block(1, &mut buffer).unwrap();
        assert_eq!(&buffer[..8], b"EFI PART");
    }

    #[test(target = "qemu")]
    fn boot_disk_write_round_trip() {
        let device = fs::test_boot_device();
        // The last two sectors before the test image's first partition are unused.
        let mut saved = [0u8; 1024];
        device.read_block(2046, &mut saved).unwrap();
        let expected = [0x5a; 1024];
        device.write_block(2046, &expected).unwrap();
        assert_eq!(device.write_block(2046, &[0xab; 513]), Err(BlockError));
        let mut result = [0u8; 1024];
        device.read_block(2046, &mut result).unwrap();
        device.write_block(2046, &saved).unwrap();
        assert_eq!(&result, &expected);
    }
}
