//! Blocking storage boundary used during the driver extraction.

/// A device or request could not complete.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BlockError;
/// Sector size used by the existing boot storage drivers.
pub const SECTOR_SIZE: usize = 512;

/// Reads fill initialized storage. A faulty implementation cannot expose uninitialized bytes.
pub trait BlockDevice: Send + Sync {
    /// Read any byte count starting at a sector, including a partial sector.
    fn read_block(&self, sector: usize, buffer: &mut [u8]) -> Result<(), BlockError>;
    /// Write a whole number of sectors.
    fn write_block(&self, sector: usize, buffer: &[u8]) -> Result<(), BlockError>;
    /// Number of addressable sectors.
    fn sector_count(&self) -> u64;
}

/// Discovered devices, initialized exactly once by boot.
#[cfg(target_os = "none")]
pub fn devices() -> &'static [alloc::sync::Arc<dyn BlockDevice>] {
    crate::legacy_drivers::BLOCK_DEVICES
        .get()
        .map_or(&[], |v| v.as_slice())
}
