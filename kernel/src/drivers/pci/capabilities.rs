use crate::drivers::pci::PciDevice;

pub struct CapabilitiesIter<'a> {
    device: &'a PciDevice,
    next_offset: u8,
    visited: u64,
}

impl<'a> CapabilitiesIter<'a> {
    pub fn new(device: &'a PciDevice) -> Self {
        let has_capabilities = device.read(0x06) & (1 << 4) != 0;
        Self {
            device,
            next_offset: if has_capabilities {
                device.read(0x34) as u8
            } else {
                0
            },
            visited: 0,
        }
    }
}

impl Iterator for CapabilitiesIter<'_> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.next_offset & !3;
        let bit = 1 << (offset / 4);
        if offset < 0x40 || self.visited & bit != 0 {
            return None;
        }

        self.visited |= bit;
        let header = self.device.read(offset);
        self.next_offset = (header >> 8) as u8;
        Some((offset, header as u8))
    }
}
