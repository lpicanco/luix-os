use core::fmt;

use crate::acpi::sdt::Sdt;

/// Multiple APIC Description Table
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct Madt {
    header: Sdt,
    local_apic_address: u32,
    flags: u32,
}

impl fmt::Display for Madt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Madt\n{}", self.header)?;
        write!(
            f,
            "\tlocal_apic_address: {:#X}\n",
            self.local_apic_address as u32
        )?;
        write!(f, "\tflags: {:#X}\n", self.flags as u32)
    }
}
