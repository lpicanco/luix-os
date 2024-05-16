use crate::println;

mod pci;
mod nvme;
pub fn init() {
    pci::init();
    nvme::init();
    println!("Drivers initialized.");
}
