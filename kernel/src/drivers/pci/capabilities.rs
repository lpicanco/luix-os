use crate::drivers::pci::PciDevice;

const PCI_CAPABILITIES: u8 = 0x34;

#[derive(Debug, Clone)]
pub struct CapabilitiesIter<'a> {
    device: &'a PciDevice,
    offset: u8,
}

impl<'a> CapabilitiesIter<'a> {
    pub fn new(device: &'a PciDevice) -> Self {
        let offset = device.read(PCI_CAPABILITIES) as u8;
        Self { device, offset }
    }
}

impl<'a> Iterator for CapabilitiesIter<'a> {
    type Item = (u8, Capability);
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == 0 {
            return None;
        }

        let current_offset = self.offset;
        self.offset = self.device.read(self.offset + 1) as u8;
        let id = self.device.read(current_offset) as u8;
        let capability_offset = current_offset + 2;

        let capability = match id {
            0x01 => Capability::PowerManagement(capability_offset),
            0x05 => Capability::Msi(capability_offset),
            0x10 => Capability::Pcie(capability_offset),
            0x11 => Capability::Msix(MsixCapability {
                message_control: self.device.read(current_offset + 2) as u16,
                table_off_and_bar: self.device.read(current_offset + 4),
                pba_off_and_bar: self.device.read(capability_offset + 8),
            }),
            _ => Capability::Other(capability_offset),
        };
        Some((current_offset, capability))
    }
}

#[derive(Clone)]
pub enum Capability {
    PowerManagement(u8),
    Pcie(u8),
    Msi(u8),
    Msix(MsixCapability),
    Other(u8),
}

#[repr(C)]
#[derive(Clone)]
pub struct MsixCapability {
    message_control: u16,
    table_off_and_bar: u32,
    pba_off_and_bar: u32,
}
