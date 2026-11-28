//! Firmware discovery is trusted. Table lengths, checksums and record offsets are checked.
use spin::Once;
pub(crate) mod madt;
use madt::Madt;

pub static ACPI: Once<Acpi> = Once::new();
pub struct Acpi {
    pub madt: Madt<'static>,
}

#[cfg(target_os = "none")]
fn firmware_bytes(address: u64, len: usize) -> Option<&'static [u8]> {
    use limine::memory_map::EntryType;
    let end = address.checked_add(len as u64)?;
    let map = crate::memory::MEMORY_MAP_REQUEST.get_response()?;
    // These ranges are never handed to the frame allocator or reclaimed by this kernel.
    let retained = map.entries().iter().any(|entry| {
        [
            EntryType::ACPI_RECLAIMABLE,
            EntryType::ACPI_NVS,
            EntryType::BOOTLOADER_RECLAIMABLE,
        ]
        .contains(&entry.entry_type)
            && address >= entry.base
            && entry
                .base
                .checked_add(entry.length)
                .is_some_and(|limit| end <= limit)
    });
    if !retained || len > isize::MAX as usize {
        return None;
    }
    let offset = crate::memory::MEMORY_MAPPER
        .get()?
        .physical_memory_offset
        .as_u64();
    let pointer = offset.checked_add(address)? as *const u8;
    // SAFETY: Limine maps this validated RAM range, which stays immutable and retained
    // after boot. Every byte pattern is valid for u8, whose alignment is one.
    Some(unsafe { core::slice::from_raw_parts(pointer, len) })
}

fn checked_table(bytes: &[u8]) -> Option<&[u8]> {
    let length = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    if length < 36 {
        return None;
    }
    let table = bytes.get(..length)?;
    (table.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0).then_some(table)
}

#[cfg(target_os = "none")]
fn table_at(address: u64) -> Option<&'static [u8]> {
    let header = firmware_bytes(address, 36)?;
    let length = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
    checked_table(firmware_bytes(address, length)?)
}

#[cfg(target_os = "none")]
pub(crate) fn init() {
    use limine::request::RsdpRequest;
    static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
    ACPI.call_once(|| {
        let offset = crate::memory::MEMORY_MAPPER
            .get()
            .unwrap()
            .physical_memory_offset
            .as_u64();
        let address = RSDP_REQUEST
            .get_response()
            .expect("RSDP response")
            .address() as u64;
        let physical = address.checked_sub(offset).unwrap_or(address);
        let rsdp = firmware_bytes(physical, 20).expect("RSDP must be retained firmware RAM");
        assert_eq!(&rsdp[..8], b"RSD PTR ");
        assert_eq!(rsdp.iter().fold(0u8, |s, b| s.wrapping_add(*b)), 0);
        // Preserve the existing ACPI 1.0/RSDT support. XSDT support is separate work.
        let rsdt_address = u32::from_le_bytes(rsdp[16..20].try_into().unwrap());
        let rsdt = table_at(rsdt_address.into()).expect("valid RSDT");
        assert_eq!(&rsdt[..4], b"RSDT");
        assert!((rsdt.len() - 36).is_multiple_of(4));
        let madt = rsdt[36..]
            .chunks_exact(4)
            .find_map(|entry| {
                let table = table_at(u32::from_le_bytes(entry.try_into().ok()?).into())?;
                Madt::parse(table)
            })
            .expect("valid MADT");
        Acpi { madt }
    });
    crate::println!("ACPI initialized.");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_short_oversized_and_bad_checksum_tables() {
        assert!(checked_table(&[]).is_none());
        let mut bytes = [0u8; 36];
        bytes[4] = 35;
        assert!(checked_table(&bytes).is_none());
        bytes[4] = 37;
        assert!(checked_table(&bytes).is_none());
        bytes[4] = 36;
        assert!(checked_table(&bytes).is_none());
        bytes[9] = 0u8.wrapping_sub(36);
        assert!(checked_table(&bytes).is_some());
    }
}
