use spin::Once;

use rsdp::Rsdp;

use crate::acpi::madt::Madt;
use crate::acpi::rsdt::Rsdt;
use crate::acpi::sdt::Signature;
use crate::{println, trace};

pub(crate) mod madt;
mod rsdp;
mod rsdt;
mod sdt;

pub static ACPI: Once<Acpi> = Once::new();

#[derive(Debug)]
pub struct Acpi {
    pub madt: Madt,
}

impl Acpi {
    fn load() -> Self {
        let rsdp = Rsdp::load();
        let rsdt = Rsdt::new(rsdp);

        let madt = rsdt
            .find_table::<Madt>(Signature::MADT)
            .expect("Failed to find MADT table");
        trace!("MADT table: {}", madt);

        Acpi { madt }
    }
}

pub(crate) fn init() {
    ACPI.call_once(Acpi::load);
    println!("ACPI initialized.");
}
