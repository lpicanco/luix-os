use core::fmt;

use limine::request::RsdpRequest;

use crate::trace;

static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

/// Root System Description Pointer
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct Rsdp {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

impl Rsdp {
    pub fn load() -> Self {
        let rsdp_address = RSDP_REQUEST
            .get_response()
            .expect("Failed to get RSDP address")
            .address() as usize;

        trace!("RSDP address: {:#X}", rsdp_address);

        let rsdp = unsafe { core::ptr::read_volatile(rsdp_address as *const Rsdp) };
        trace!("RSDP loaded: {}", rsdp);
        rsdp
    }

    pub fn sdt_address(&self) -> u32 {
        /*
         * TODO: Make it compatible with ACPI 2.0+
         */
        self.rsdt_address
    }
}

impl fmt::Display for Rsdp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rsdp\n")?;
        write!(
            f,
            "\tsignature: {}\n",
            core::str::from_utf8(&self.signature).unwrap()
        )?;
        write!(f, "\tchecksum: {:#X}\n", self.checksum)?;
        write!(
            f,
            "\toem_id: {}\n",
            core::str::from_utf8(&self.oem_id).unwrap()
        )?;
        write!(f, "\trevision: {:#X}\n", self.revision)?;
        write!(f, "\trsdt_address: {:#X}\n", self.rsdt_address as u32)
    }
}
