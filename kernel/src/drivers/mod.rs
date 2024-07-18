use crate::println;
use core::mem::MaybeUninit;

mod pci;
pub(crate) mod nvme;
pub(crate) mod fs;
pub mod keyboard;

pub fn init() {
    pci::init();
    nvme::init();
    fs::init();
    println!("Drivers initialized.");
}

pub trait BlockDevice: Send + Sync{
    fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> usize;
    fn write_block(&self, sector: usize, buffer: &[u8]) -> usize;
}
