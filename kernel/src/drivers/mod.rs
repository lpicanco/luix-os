use crate::println;

mod pci;
pub fn init() {
    pci::init();
    println!("Drivers initialized.");
}
