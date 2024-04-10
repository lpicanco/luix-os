use core::{fmt, ptr};
use core::mem::size_of;

use crate::acpi::sdt::{AcpiTable, Sdt};

/// Multiple APIC Description Table
#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct Madt {
    header: Sdt,
    pub local_apic_address: u32,
    flags: u32,
    madt_address: u32,
}

impl Madt {
    pub fn iter_entries(&self) -> MadtEntryIterator {
        let madt_len = size_of::<Sdt>() + size_of::<u32>() + size_of::<u32>();

        let ptr = self.madt_address as *mut Madt;
        let entries_addr = (ptr as *const () as usize) + madt_len;

        MadtEntryIterator {
            ptr: entries_addr as *const MadtEntryHeader,
            remaining_len: self.header.length as usize - madt_len,
        }
    }

}

impl AcpiTable for Madt {
    fn load_from_address<T>(address: u32) -> Self {
        let mut madt = Sdt::load_from_address::<Madt>(address);
        madt.madt_address = address;

        madt
    }
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

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

pub enum MadtEntry {
    LocalApic(MadtLocalApicEntry),
    IoApic(MadtIoApicEntry),
    Unknown(MadtEntryHeader),
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct MadtLocalApicEntry {
    header: MadtEntryHeader,
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    flags: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct MadtIoApicEntry {
    header: MadtEntryHeader,
    pub io_apic_id: u8,
    reserved: u8,
    pub io_apic_address: u32,
    global_system_interrupt_base: u32,
}

pub struct MadtEntryIterator {
    ptr: *const MadtEntryHeader,
    remaining_len: usize,
}

impl Iterator for MadtEntryIterator {
    type Item = MadtEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining_len == 0 {
            return None;
        }

        let header = unsafe { ptr::read_volatile(self.ptr) };
        let entry = match header.entry_type {
            0 => MadtEntry::LocalApic(unsafe { ptr::read_volatile(self.ptr as *const MadtLocalApicEntry) }),
            1 => MadtEntry::IoApic(unsafe { ptr::read_volatile(self.ptr as *const MadtIoApicEntry) }),
            _ => MadtEntry::Unknown(header),
        };

        self.ptr = unsafe { self.ptr.byte_add(header.length as usize) };
        self.remaining_len -= header.length as usize;

        Some(entry)
    }
}