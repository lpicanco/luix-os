use alloc::vec::Vec;
use core::fmt;

use spin::Once;

use crate::arch::port;
use crate::bits::Bits;
use crate::drivers::pci::capabilities::CapabilitiesIter;
use crate::memory::address::PhysicalAddress;
use crate::println;

mod capabilities;

pub static PCI_DRIVER: Once<PciDriver> = Once::new();

pub(crate) fn init() {
    let mut driver = PciDriver {
        scanner: PciScanner::new(),
    };
    driver.scanner.scan_devices();
    PCI_DRIVER.call_once(|| driver);

    println!(
        "PCI Devices scanned. Found {} devices.",
        PCI_DRIVER.get().unwrap().devices().len()
    );
}

pub struct PciDriver {
    scanner: PciScanner,
}

impl PciDriver {
    const DEVICE_CLASS_STORAGE: u8 = 0x01;

    pub fn devices(&self) -> &Vec<PciDevice> {
        &self.scanner.devices
    }

    pub fn storage_devices(&self) -> Vec<&PciDevice> {
        self.scanner
            .devices
            .iter()
            .filter(|device| device.class_code == Self::DEVICE_CLASS_STORAGE)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct PciDevice {
    bus: u8,
    device: u8,
    function: u8,
    vendor_id: u16,
    device_id: u16,
    class_code: u8,
    pub subclass: u8,
    bars: [u32; 6],
    header_type: u8,
}

impl PciDevice {
    pub fn enable_bus_mastering(&self) {
        let mut command = self.read(0x04);
        command.set_bit(2, true);
        self.write(0x04, command);
    }

    pub fn enable_mmio(&self) {
        let mut command = self.read(0x04);
        command.set_bit(1, true);
        self.write(0x04, command);
    }

    pub fn capabilities(&self) -> CapabilitiesIter {
        CapabilitiesIter::new(self)
    }

    pub fn read_bar0_memory_address(&self) -> PhysicalAddress {
        // TODO: implement macro for reading bar memory address
        self.read_bar_memory_address(0)
    }

    pub fn read(&self, offset: u8) -> u32 {
        pci_read(
            u16::from(self.bus),
            u16::from(self.device),
            u16::from(self.function),
            offset,
        )
    }

    pub fn write(&self, offset: u8, value: u32) {
        pci_write(
            u16::from(self.bus),
            u16::from(self.device),
            u16::from(self.function),
            offset,
            value,
        );
    }

    fn read_bar_memory_address(&self, index: usize) -> PhysicalAddress {
        let mut bar_lower = self.bars[index];

        // Determine the type of the BAR(32-bit or 64-bit). If bits 2 and 1 are equal to 0x0, then it's a 32-bit address. 0x2 means 64-bit address.
        let bar_type = bar_lower.get_bits(1..3);

        // Clear the last 4 bits from the lower bar since they are not part of the address
        bar_lower &= 0xFFFF_FFF0;

        if bar_type == 0b00 {
            return PhysicalAddress::new(u64::from(bar_lower));
        }

        // If it's a 64-bit address, read the upper 32 bits from the next BAR
        let bar_upper = self.bars[index + 1];
        PhysicalAddress::new((u64::from(bar_upper) << 32) | u64::from(bar_lower))
    }
}

impl fmt::Display for PciDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeviceId: {:02x}:{:02x}.{:x} Vendor: {:04x} Device: {:04x} Class: {:02x} Sub class: {:02x}",
               self.bus, self.device, self.function, self.vendor_id, self.device_id, self.class_code, self.subclass)
    }
}

struct PciScanner {
    pub devices: Vec<PciDevice>,
}

impl PciScanner {
    pub fn new() -> Self {
        PciScanner {
            devices: Vec::new(),
        }
    }

    pub fn scan_devices(&mut self) {
        for bus in 0..256u16 {
            for device in 0..32 {
                for function in 0..8 {
                    let vendor_id = self.read(bus, device, function, 0) as u16;
                    if vendor_id == 0xFFFF {
                        // Device doesn't exist
                        continue;
                    }

                    let device_id = self.read(bus, device, function, 2) as u16;
                    let class_code = self.read(bus, device, function, 0xB) as u8;
                    let subclass = self.read(bus, device, function, 0xA) as u8;
                    let header_type = self.read(bus, device, function, 0xE) as u8;
                    let bars = [
                        self.read(bus, device, function, 0x10),
                        self.read(bus, device, function, 0x14),
                        self.read(bus, device, function, 0x18),
                        self.read(bus, device, function, 0x1C),
                        self.read(bus, device, function, 0x20),
                        self.read(bus, device, function, 0x24),
                    ];
                    let device = PciDevice {
                        bus: bus as u8,
                        device: device as u8,
                        function: function as u8,
                        vendor_id,
                        device_id,
                        class_code,
                        subclass,
                        header_type,
                        bars,
                    };
                    println!("Found PCI Device: {}", device);
                    self.devices.push(device);
                }
            }
        }
    }

    fn read(&self, bus: u16, device: u16, function: u16, offset: u8) -> u32 {
        return pci_read(bus, device, function, offset);
    }
}

fn pci_read(bus: u16, device: u16, function: u16, offset: u8) -> u32 {
    let address = get_pci_address(bus, device, function, offset);
    port::write_32(0xCF8, address);
    port::read_32(0xCFC) >> ((offset & (!0xFC)) * 8)
}

fn pci_write(bus: u16, device: u16, function: u16, offset: u8, value: u32) {
    let address = get_pci_address(bus, device, function, offset);
    port::write_32(0xCF8, address);
    port::write_32(0xCFC, value);
}

fn get_pci_address(bus: u16, device: u16, function: u16, offset: u8) -> u32 {
    (u32::from(bus) << 16)
        | (u32::from(device) << 11)
        | (u32::from(function) << 8)
        | (u32::from(offset) & u32::from(0xFCu32))
        | 0x8000_0000
}

#[cfg(test)]
mod tests {
    use core::assert_matches::assert_matches;

    use super::*;

    #[test_case]
    fn test_pci_scanner() {
        let mut scanner = PciScanner::new();
        scanner.scan_devices();
        assert_matches!(
            scanner
                .devices
                .iter()
                .find(|device| device.class_code == 0x01),
            Some(_)
        );
    }

    #[test_case]
    fn test_pci_driver() {
        let driver = PCI_DRIVER.get().unwrap();
        assert_ne!(driver.devices().len(), 0);
        assert_ne!(driver.storage_devices().len(), 0);
        assert_matches!(
            driver
                .storage_devices()
                .iter()
                .find(|device| device.class_code == 0x01),
            Some(_)
        );
    }
}
