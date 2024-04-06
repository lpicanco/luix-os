use rsdp::Rsdp;

use crate::acpi::madt::Madt;
use crate::acpi::rsdt::{Rsdt, Signature};
use crate::{println, trace};

mod madt;
mod rsdp;
mod rsdt;
mod sdt;

struct Acpi {}

impl Acpi {
    fn load() -> Self {
        let rsdp = Rsdp::load();
        let rsdt = Rsdt::new(rsdp);

        let madt = rsdt
            .find_table::<Madt>(Signature::MADT)
            .expect("Failed to find MADT table");
        trace!("MADT table: {}", madt);

        Acpi {}
    }
}

pub(crate) fn init() {
    let _ = Acpi::load();
    println!("ACPI initialized.");
}
