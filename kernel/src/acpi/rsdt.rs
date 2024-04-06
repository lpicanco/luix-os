use core::mem::size_of;
use core::{fmt, slice};

use crate::acpi::rsdp::Rsdp;
use crate::acpi::sdt::Sdt;
use crate::trace;

/// Root System Description Table
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct Rsdt {
    header: Sdt,
    pub tables: &'static [u32],
}

impl Rsdt {
    pub fn new(rsdp: Rsdp) -> Self {
        let sdt = Sdt::load_from_address::<Sdt>(rsdp.sdt_address());

        let ptr = rsdp.sdt_address() as *mut Sdt;
        let tables_addr = (ptr as *const () as usize) + size_of::<Sdt>();
        let tables_len = sdt.length as usize - size_of::<Sdt>();
        let num_entries = tables_len / size_of::<u32>();
        let tables = unsafe { slice::from_raw_parts(tables_addr as *const u32, num_entries) };

        let rsp = Self {
            header: Sdt::load_from_address(rsdp.sdt_address()),
            tables,
        };

        trace!(
            "RSDT loaded from address {:#X}: {}",
            rsdp.sdt_address(),
            rsp
        );
        rsp
    }
    pub fn find_table<T>(&self, signature: Signature) -> Option<T> {
        trace!("Searching for table: {}", signature);

        self.tables.iter().find_map(|&ptr| {
            let sdt = Sdt::load_from_address::<Sdt>(ptr);
            if sdt.signature == signature {
                Some(Sdt::load_from_address::<T>(ptr))
            } else {
                None
            }
        })
    }
}
impl fmt::Display for Rsdt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rsdt\n{}", self.header)?;
        write!(f, "\ttables_count: {}\n", self.tables.len())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Signature([u8; 4]);

impl Signature {
    pub const MADT: Signature = Signature(*b"APIC");
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", core::str::from_utf8(&self.0).unwrap())
    }
}
