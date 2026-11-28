#[derive(Clone, Copy)]
pub struct Madt<'a> {
    pub local_apic_address: u32,
    entries: &'a [u8],
}
impl<'a> Madt<'a> {
    pub fn parse(table: &'a [u8]) -> Option<Self> {
        if table.get(..4)? != b"APIC" {
            return None;
        }
        Some(Self {
            local_apic_address: u32::from_le_bytes(table.get(36..40)?.try_into().ok()?),
            entries: table.get(44..)?,
        })
    }
    pub fn iter_entries(&self) -> MadtEntryIterator<'a> {
        MadtEntryIterator(self.entries)
    }
}
pub enum MadtEntry {
    LocalApic(MadtLocalApicEntry),
    IoApic(MadtIoApicEntry),
    Unknown,
}
pub struct MadtLocalApicEntry {
    pub acpi_processor_id: u8,
    pub apic_id: u8,
}
pub struct MadtIoApicEntry {
    pub io_apic_id: u8,
    pub io_apic_address: u32,
}
pub struct MadtEntryIterator<'a>(&'a [u8]);
impl Iterator for MadtEntryIterator<'_> {
    type Item = MadtEntry;
    fn next(&mut self) -> Option<Self::Item> {
        let bytes = core::mem::take(&mut self.0);
        let length = usize::from(*bytes.get(1)?);
        if length < 2 {
            return None;
        }
        let entry = bytes.get(..length)?;
        self.0 = &bytes[length..];
        Some(match entry[0] {
            0 if length >= 8 => MadtEntry::LocalApic(MadtLocalApicEntry {
                acpi_processor_id: entry[2],
                apic_id: entry[3],
            }),
            1 if length >= 12 => MadtEntry::IoApic(MadtIoApicEntry {
                io_apic_id: entry[2],
                io_apic_address: u32::from_le_bytes(entry[4..8].try_into().ok()?),
            }),
            _ => MadtEntry::Unknown,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_entries_terminate_without_reading_past_table() {
        for bytes in [&[0u8][..], &[0, 0], &[0, 1], &[1, 12, 0], &[0, 255]] {
            let mut iter = MadtEntryIterator(bytes);
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }
        assert!(matches!(
            MadtEntryIterator(&[0, 2]).next(),
            Some(MadtEntry::Unknown)
        ));
    }
}
