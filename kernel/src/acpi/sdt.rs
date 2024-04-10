use core::{fmt, ptr, str};

pub trait AcpiTable {
    fn load_from_address<T>(address: u32) -> Self;
}

/// System Description Table
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct Sdt {
    pub(crate) signature: Signature,
    pub(crate) length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

impl Sdt {
    pub fn load_from_address<T>(address: u32) -> T {
        unsafe { ptr::read_volatile(address as *const T) }
    }
}

impl fmt::Display for Sdt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sdt\n")?;
        write!(f, "\tsignature: {}\n", self.signature)?;
        write!(f, "\tlength: {:#X}\n", self.length as u32)?;
        write!(f, "\trevision: {:#X}\n", self.revision)?;
        write!(f, "\tchecksum: {:#X}\n", self.checksum)?;
        write!(f, "\toem_id: {}\n", str::from_utf8(&self.oem_id).unwrap())?;
        write!(
            f,
            "\toem_table_id: {}\n",
            str::from_utf8(&self.oem_table_id).unwrap()
        )?;
        write!(f, "\toem_revision: {:#X}\n", self.oem_revision as u32)?;
        write!(f, "\tcreator_id: {:#X}\n", self.creator_id as u32)?;
        write!(
            f,
            "\tcreator_revision: {:#X}\n",
            self.creator_revision as u32
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Signature([u8; 4]);

impl Signature {
    pub const MADT: Signature = Signature(*b"APIC");
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", str::from_utf8(&self.0).unwrap())
    }
}
