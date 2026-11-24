use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::mem::{offset_of, size_of, MaybeUninit};
use core::ptr;
use core::sync::atomic::{fence, Ordering};

use spin::Mutex;

use super::pci::{PciDevice, PCI_DRIVER};
use super::{BlockDevice, BlockError, SECTOR_SIZE};
use crate::memory::allocator::dma_allocator::Dma;
use crate::println;

mod transport;
use transport::Transport;

const VERSION_1: u64 = 1 << 32;
const READ_ONLY: u64 = 1 << 5;

const ACKNOWLEDGE: u8 = 1;
const DRIVER: u8 = 2;
const DRIVER_OK: u8 = 4;
const FEATURES_OK: u8 = 8;
const NEEDS_RESET: u8 = 64;
const FAILED: u8 = 128;

const RESET_SPINS: usize = 1_000_000;
const QUEUE_SIZE: usize = 4;
const DESC_NEXT: u16 = 1;
const DESC_WRITE: u16 = 2;
const REQUEST_READ: u32 = 0;
const REQUEST_WRITE: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone)]
struct Descriptor {
    address: u64,
    length: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct Available {
    flags: u16,
    index: u16,
    ring: [u16; QUEUE_SIZE],
    used_event: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct UsedEntry {
    id: u32,
    length: u32,
}

#[repr(C)]
struct Used {
    flags: u16,
    index: u16,
    ring: [UsedEntry; QUEUE_SIZE],
    avail_event: u16,
}

#[repr(C)]
struct RequestHeader {
    operation: u32,
    reserved: u32,
    sector: u64,
}

// One contiguous DMA allocation holds the split ring and its one in-flight request.
#[repr(C, align(16))]
struct QueueMemory {
    descriptors: [Descriptor; QUEUE_SIZE],
    available: Available,
    used: Used,
    header: RequestHeader,
    data: [u8; SECTOR_SIZE],
    status: u8,
}

struct RequestQueue {
    transport: Transport,
    memory: Dma<UnsafeCell<QueueMemory>>,
    index: u16,
}

impl RequestQueue {
    fn new(transport: Transport) -> Option<Self> {
        let mut queue = Self {
            transport,
            memory: Dma::zeroed(),
            index: 0,
        };
        queue.transport.select_queue(0);
        let base = queue.memory.addr().as_u64();
        let mem = queue.memory.get();
        // DMA is still disabled for this queue. Descriptor zero heads every request.
        unsafe {
            (*mem).available.flags = 1; // VRING_AVAIL_F_NO_INTERRUPT
            (*mem).descriptors[0] = Descriptor {
                address: base + offset_of!(QueueMemory, header) as u64,
                length: size_of::<RequestHeader>() as u32,
                flags: DESC_NEXT,
                next: 1,
            };
            (*mem).descriptors[1] = Descriptor {
                address: base + offset_of!(QueueMemory, data) as u64,
                length: SECTOR_SIZE as u32,
                flags: DESC_NEXT,
                next: 2,
            };
            (*mem).descriptors[2] = Descriptor {
                address: base + offset_of!(QueueMemory, status) as u64,
                length: 1,
                flags: DESC_WRITE,
                next: 0,
            };
        }
        fence(Ordering::Release);
        queue.transport.configure_queue(
            QUEUE_SIZE as u16,
            base + offset_of!(QueueMemory, descriptors) as u64,
            base + offset_of!(QueueMemory, available) as u64,
            base + offset_of!(QueueMemory, used) as u64,
        )?;
        Some(queue)
    }

    fn data(&self) -> *mut u8 {
        unsafe { (&raw mut (*self.memory.get()).data).cast() }
    }

    fn transfer(&mut self, operation: u32, sector: u64) -> Result<(), BlockError> {
        let mem = self.memory.get();
        let slot = usize::from(self.index) % QUEUE_SIZE;
        let next_index = self.index.wrapping_add(1);
        if self.transport.status() & (NEEDS_RESET | FAILED) != 0 {
            return Err(BlockError);
        }

        // The mutex serializes submitters. The device owns the buffers between
        // publication of the available index and observation of the used index.
        unsafe {
            (*mem).header = RequestHeader {
                operation,
                reserved: 0,
                sector,
            };
            (*mem).descriptors[1].flags = if operation == REQUEST_READ {
                DESC_NEXT | DESC_WRITE
            } else {
                DESC_NEXT
            };
            (&raw mut (*mem).status).write_volatile(0xff);
            (*mem).available.ring[slot] = 0;
            fence(Ordering::Release);
            (&raw mut (*mem).available.index).write_volatile(next_index);
            fence(Ordering::SeqCst);
            self.transport.notify_queue(0);

            let index = loop {
                let index = (&raw const (*mem).used.index).read_volatile();
                if index != self.index {
                    break index;
                }
                if self.transport.status() & (NEEDS_RESET | FAILED) != 0 {
                    return Err(BlockError);
                }
                spin_loop();
            };
            fence(Ordering::Acquire);
            let completion = (&raw const (*mem).used.ring[slot]).read_volatile();
            if index != next_index || completion.id != 0 {
                self.transport.set_status(self.transport.status() | FAILED);
                return Err(BlockError);
            }
            self.index = next_index;
            // Devices are known to report used lengths inaccurately, so only the status counts.
            if (&raw const (*mem).status).read_volatile() != 0 {
                return Err(BlockError);
            }
        }
        Ok(())
    }

    fn check_range(&self, sector: usize, bytes: usize) -> Result<(), BlockError> {
        let sectors = bytes.div_ceil(SECTOR_SIZE) as u64;
        let end = (sector as u64).checked_add(sectors).ok_or(BlockError)?;
        if self.transport.status() & (NEEDS_RESET | FAILED) != 0 || end > self.transport.capacity()
        {
            return Err(BlockError);
        }
        Ok(())
    }
}

fn reset(transport: &Transport) -> bool {
    transport.set_status(0);
    for _ in 0..RESET_SPINS {
        if transport.status() == 0 {
            return true;
        }
        spin_loop();
    }
    false
}

impl Drop for RequestQueue {
    fn drop(&mut self) {
        // The device must release the rings before their frames can be reused.
        if !reset(&self.transport) {
            println!("Virtio block device did not acknowledge reset");
        }
    }
}

pub(super) struct VirtioBlockDevice {
    queue: Mutex<RequestQueue>,
    read_only: bool,
}

#[cfg(test)]
static TEST_DEVICES: spin::Once<Vec<Arc<VirtioBlockDevice>>> = spin::Once::new();

pub(super) fn init() -> Vec<Arc<dyn BlockDevice>> {
    let devices: Vec<Arc<VirtioBlockDevice>> = PCI_DRIVER
        .get()
        .unwrap()
        .devices()
        .iter()
        .filter_map(probe)
        .map(Arc::new)
        .collect();
    for device in &devices {
        println!(
            "Virtio block device initialized. Total capacity: {}MB.",
            device.sector_count() * SECTOR_SIZE as u64 / 1024 / 1024
        );
    }
    #[cfg(test)]
    TEST_DEVICES.call_once(|| devices.clone());
    devices
        .into_iter()
        .map(|device| device as Arc<dyn BlockDevice>)
        .collect()
}

fn probe(device: &PciDevice) -> Option<VirtioBlockDevice> {
    let transport = Transport::probe(device)?;
    if !reset(&transport) {
        return None;
    }
    transport.set_status(ACKNOWLEDGE);
    transport.set_status(ACKNOWLEDGE | DRIVER);
    let features = transport.device_features() & (VERSION_1 | READ_ONLY);
    if features & VERSION_1 == 0 {
        transport.set_status(ACKNOWLEDGE | DRIVER | FAILED);
        return None;
    }
    transport.set_driver_features(features);
    transport.set_status(ACKNOWLEDGE | DRIVER | FEATURES_OK);
    if transport.status() & FEATURES_OK == 0 {
        transport.set_status(ACKNOWLEDGE | DRIVER | FAILED);
        return None;
    }
    device.enable_bus_mastering();
    let queue = RequestQueue::new(transport)?;
    queue
        .transport
        .set_status(ACKNOWLEDGE | DRIVER | FEATURES_OK | DRIVER_OK);
    Some(VirtioBlockDevice {
        queue: Mutex::new(queue),
        read_only: features & READ_ONLY != 0,
    })
}

// SAFETY: each successful read copies all requested bytes from completed input descriptors.
unsafe impl BlockDevice for VirtioBlockDevice {
    fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> Result<(), BlockError> {
        if buffer.is_empty() {
            return Ok(());
        }
        let mut queue = self.queue.lock();
        queue.check_range(sector, buffer.len())?;
        for (i, chunk) in buffer.chunks_mut(SECTOR_SIZE).enumerate() {
            queue.transfer(REQUEST_READ, (sector + i) as u64)?;
            unsafe {
                ptr::copy_nonoverlapping(queue.data().cast(), chunk.as_mut_ptr(), chunk.len())
            };
        }
        Ok(())
    }

    fn write_block(&self, sector: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if self.read_only || !buffer.len().is_multiple_of(SECTOR_SIZE) {
            return Err(BlockError);
        }
        if buffer.is_empty() {
            return Ok(());
        }
        let mut queue = self.queue.lock();
        queue.check_range(sector, buffer.len())?;
        for (i, chunk) in buffer.chunks(SECTOR_SIZE).enumerate() {
            unsafe { ptr::copy_nonoverlapping(chunk.as_ptr(), queue.data(), SECTOR_SIZE) };
            queue.transfer(REQUEST_WRITE, (sector + i) as u64)?;
        }
        Ok(())
    }

    fn sector_count(&self) -> u64 {
        self.queue.lock().transport.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test(target = "qemu")]
    fn device_error_completes_request_and_next_read_succeeds() {
        let Some(device) = TEST_DEVICES.get().and_then(|devices| devices.first()) else {
            return;
        };
        let mut queue = device.queue.lock();
        let end = queue.transport.capacity();
        assert_eq!(queue.transfer(REQUEST_READ, end), Err(BlockError));
        queue.transfer(REQUEST_READ, 1).unwrap();
        let data = unsafe { core::slice::from_raw_parts(queue.data(), 8) };
        assert_eq!(data, b"EFI PART");
    }
}
