use crate::arch::memory::paging::Page;
use crate::legacy_drivers::pci::PciDevice;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;
use crate::memory::MEMORY_MAPPER;

// Offsets in struct virtio_pci_common_cfg (virtio 1.2, section 4.1.4.3).
const DEVICE_FEATURE_SELECT: u64 = 0;
const DEVICE_FEATURE: u64 = 4;
const DRIVER_FEATURE_SELECT: u64 = 8;
const DRIVER_FEATURE: u64 = 12;
const DEVICE_STATUS: u64 = 20;
const CONFIG_GENERATION: u64 = 21;
const QUEUE_SELECT: u64 = 22;
const QUEUE_SIZE: u64 = 24;
const QUEUE_ENABLE: u64 = 28;
const QUEUE_NOTIFY_OFF: u64 = 30;
const QUEUE_DESC: u64 = 32;
const QUEUE_DRIVER: u64 = 40;
const QUEUE_DEVICE: u64 = 48;
const COMMON_LENGTH: u32 = 56;

// The result of sizing a BAR: its base and length, or `None` when it is not usable memory.
type Bar = Option<(u64, u64)>;

#[derive(Clone, Copy)]
struct Region {
    physical: u64,
    length: u32,
}

impl Region {
    fn from_cap(device: &PciDevice, cap: u8, bars: &mut [Option<Bar>; 6]) -> Option<Self> {
        let index = device.read(cap + 4) as u8;
        let bar = bars.get_mut(usize::from(index))?;
        let (base, bar_size) = (*bar.get_or_insert_with(|| device.memory_bar(index)))?;
        let offset = u64::from(device.read(cap + 8));
        let length = device.read(cap + 12);
        if offset + u64::from(length) > bar_size {
            return None;
        }
        Some(Self {
            physical: base.checked_add(offset)?,
            length,
        })
    }

    fn map(&self, offset: u64, len: u32, alignment: u64) -> Option<u64> {
        if offset.checked_add(u64::from(len))? > u64::from(self.length) {
            return None;
        }
        let physical = self.physical.checked_add(offset)?;
        if !physical.is_multiple_of(alignment) {
            return None;
        }
        let mapper = MEMORY_MAPPER.get()?;
        let direct_map_base = mapper.physical_memory_offset.as_u64();
        let start = direct_map_base.checked_add(physical)?;
        let end = start.checked_add(u64::from(len) - 1)?;
        for page in Page::range_inclusive(
            Page::containing_address(VirtualAddress::new(start)),
            Page::containing_address(VirtualAddress::new(end)),
        ) {
            let addr = PhysicalAddress::new(page.start_address.as_u64() - direct_map_base);
            mapper.map_mmio_page(page, PhysicalFrame::containing_address(addr))?;
        }
        Some(start)
    }
}

pub(super) struct Transport {
    common: u64,
    device: u64,
    notifications: Region,
    notify_multiplier: u32,
    doorbell: Option<u64>,
}

impl Transport {
    pub(super) fn probe(device: &PciDevice) -> Option<Self> {
        if device.vendor_id != 0x1af4 || !matches!(device.device_id, 0x1001 | 0x1042) {
            return None;
        }
        let mut common = None;
        let mut config = None;
        let mut notifications = None;
        let mut bars = [None; 6];
        for (cap, id) in device.capabilities() {
            if id != 0x09 {
                continue;
            }
            let length = device.read(cap + 2) as u8;
            if length < 16 || u16::from(cap) + u16::from(length) > 256 {
                continue;
            }
            match device.read(cap + 3) as u8 {
                1 if common.is_none() => {
                    common = Region::from_cap(device, cap, &mut bars)
                        .and_then(|r| r.map(0, COMMON_LENGTH, 4));
                }
                2 if length >= 20 && notifications.is_none() => {
                    notifications = Region::from_cap(device, cap, &mut bars)
                        .map(|region| (region, device.read(cap + 16)));
                }
                4 if config.is_none() => {
                    config = Region::from_cap(device, cap, &mut bars).and_then(|r| r.map(0, 8, 4));
                }
                _ => {}
            }
        }
        let (notifications, notify_multiplier) = notifications?;
        let transport = Self {
            common: common?,
            device: config?,
            notifications,
            notify_multiplier,
            doorbell: None,
        };
        device.enable_mmio();
        device.disable_legacy_interrupts();
        Some(transport)
    }

    fn read<T: Copy>(&self, offset: u64) -> T {
        // SAFETY: discovery checked the BAR extent and alignment. Private callers use
        // fixed register offsets with the matching integer width.
        unsafe { ((self.common + offset) as *const T).read_volatile() }
    }

    fn write<T>(&self, offset: u64, value: T) {
        // SAFETY: discovery checked the BAR extent and alignment. Private callers use
        // fixed register offsets with the matching integer width.
        unsafe { ((self.common + offset) as *mut T).write_volatile(value) }
    }

    pub(super) fn status(&self) -> u8 {
        self.read(DEVICE_STATUS)
    }

    pub(super) fn set_status(&self, status: u8) {
        self.write(DEVICE_STATUS, status);
    }

    pub(super) fn device_features(&self) -> u64 {
        self.write(DEVICE_FEATURE_SELECT, 0u32);
        let low: u32 = self.read(DEVICE_FEATURE);
        self.write(DEVICE_FEATURE_SELECT, 1u32);
        let high: u32 = self.read(DEVICE_FEATURE);
        u64::from(low) | (u64::from(high) << 32)
    }

    pub(super) fn set_driver_features(&self, features: u64) {
        self.write(DRIVER_FEATURE_SELECT, 0u32);
        self.write(DRIVER_FEATURE, features as u32);
        self.write(DRIVER_FEATURE_SELECT, 1u32);
        self.write(DRIVER_FEATURE, (features >> 32) as u32);
    }

    pub(super) fn select_queue(&self, index: u16) {
        self.write(QUEUE_SELECT, index);
    }

    pub(super) fn queue_size(&self) -> u16 {
        self.read(QUEUE_SIZE)
    }

    pub(super) fn configure_queue(
        &mut self,
        size: u16,
        desc: u64,
        avail: u64,
        used: u64,
    ) -> Option<()> {
        if size == 0 || size > self.queue_size() || self.read::<u16>(QUEUE_ENABLE) != 0 {
            return None;
        }
        let notify_off: u16 = self.read(QUEUE_NOTIFY_OFF);
        let offset = u64::from(notify_off) * u64::from(self.notify_multiplier);
        self.doorbell = Some(self.notifications.map(offset, 2, 2)?);
        self.write(QUEUE_SIZE, size);
        for (offset, address) in [
            (QUEUE_DESC, desc),
            (QUEUE_DRIVER, avail),
            (QUEUE_DEVICE, used),
        ] {
            self.write(offset, address as u32);
            self.write(offset + 4, (address >> 32) as u32);
        }
        self.write(QUEUE_ENABLE, 1u16);
        Some(())
    }

    pub(super) fn notify_queue(&self, index: u16) {
        let address = self.doorbell.expect("queue has been enabled");
        // SAFETY: queue configuration checked the extent and alignment of this u16 doorbell.
        unsafe { (address as *mut u16).write_volatile(index) }
    }

    pub(super) fn capacity(&self) -> u64 {
        loop {
            let generation: u8 = self.read(CONFIG_GENERATION);
            // SAFETY: discovery checked this aligned u32 register within the capacity field.
            let low = unsafe { (self.device as *const u32).read_volatile() };
            // SAFETY: discovery validated the aligned eight byte capacity register.
            let high = unsafe { ((self.device + 4) as *const u32).read_volatile() };
            if generation == self.read::<u8>(CONFIG_GENERATION) {
                return u64::from(low) | (u64::from(high) << 32);
            }
        }
    }
}
