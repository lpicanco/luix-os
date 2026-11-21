#[cfg(target_os = "none")]
use crate::println;
use core::mem::MaybeUninit;

pub(crate) mod fs;
#[cfg(target_os = "none")]
pub mod keyboard;
#[cfg(target_os = "none")]
pub(crate) mod nvme;
#[cfg(target_os = "none")]
mod pci;

#[cfg(target_os = "none")]
pub fn init() {
    pci::init();
    nvme::init();
    fs::init();
    println!("Drivers initialized.");
}

pub trait BlockDevice: Send + Sync {
    fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> usize;
    fn write_block(&self, sector: usize, buffer: &[u8]) -> usize;
}
