use crate::println;

pub(crate) mod io_apic;
pub(crate) mod local_apic;

pub(crate) fn init() {
    io_apic::init();
    local_apic::init();
    println!("APIC initialized.");
}
