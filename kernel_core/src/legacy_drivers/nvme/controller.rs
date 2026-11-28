use alloc::vec::Vec;
use core::{cell::UnsafeCell, hint::spin_loop, ptr};

use spin::Mutex;

use crate::arch::memory::paging::Page;
use crate::legacy_drivers::nvme::command::{
    IdentifyController, IdentifyNamespace, NvmeAdminIdentifyCns, NvmeCommand, NvmeIoCommand,
    MAX_TRANSFER,
};
use crate::legacy_drivers::nvme::queue::QueueGroup;
use crate::legacy_drivers::{BlockDevice, BlockError, SECTOR_SIZE};
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::allocator::dma_allocator::Dma;
use crate::memory::frame::PhysicalFrame;
use crate::memory::MEMORY_MAPPER;
use crate::trace;

const NVME_QUEUE_SIZE: usize = 10;

/// NVMe Controller
/// Specification: https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2.0d-2024.01.11-Ratified.pdf
pub(crate) struct NvmeController {
    base_address: PhysicalAddress,
    mapped: bool,
    admin_queue: Mutex<QueueGroup>,
    io_queue: Mutex<QueueGroup>,
    pub namespaces: Vec<Namespace>,
    pub identify_controller: Option<IdentifyController>,
}

impl NvmeController {
    const QUEUE_ADDR_OFFSET: usize = 0x1000; // Offset from the base address where the queues are located
    pub fn new(base_address: PhysicalAddress) -> Self {
        let queue_size = NVME_QUEUE_SIZE;
        let queue_base_address = base_address.as_u64() as usize + Self::QUEUE_ADDR_OFFSET;

        NvmeController {
            base_address,
            mapped: false,
            admin_queue: Mutex::new(QueueGroup::new(0, queue_size, queue_base_address)),
            io_queue: Mutex::new(QueueGroup::new(1, queue_size, queue_base_address)),
            namespaces: Vec::new(),
            identify_controller: None,
        }
    }

    pub(crate) fn init(&mut self) -> Result<(), u16> {
        self.map_address();

        self.mapped = true;
        self.set_enabled(false);
        let caps = self.read64(0);
        // This transitional driver supports NVM commands, 4 KiB pages, and a 4-byte doorbell stride.
        if caps & 0xffff < (NVME_QUEUE_SIZE - 1) as u64
            || (caps >> 32) & 0xf != 0
            || (caps >> 48) & 0xf != 0
            || caps & (1 << 37) == 0
        {
            return Err(u16::MAX);
        }
        let admin_queue = self.admin_queue.lock();
        self.write32(
            0x24,
            ((admin_queue.queue_size - 1) << 16 | (admin_queue.queue_size - 1)) as u32,
        );
        self.write64(0x28, admin_queue.submission_queue_addr().as_u64());
        self.write64(0x30, admin_queue.completion_queue_addr().as_u64());
        self.write32(0x14, (6 << 16) | (4 << 20));
        drop(admin_queue);
        self.set_enabled(true);

        let result = self.initialize_queues_and_namespaces();
        if result.is_err() || !self.has_usable_namespace() {
            // Stop DMA before the caller releases this controller's queues.
            self.set_enabled(false);
        }
        result
    }

    pub(crate) fn has_usable_namespace(&self) -> bool {
        self.namespaces
            .first()
            .is_some_and(|namespace| namespace.block_size == SECTOR_SIZE)
    }

    fn initialize_queues_and_namespaces(&mut self) -> Result<(), u16> {
        let identify_controller = self.identify_controller()?;
        trace!("Identify controller: {:?}", identify_controller);
        self.identify_controller = Some(identify_controller);

        self.namespaces = self.identify_namespaces()?;
        trace!("Namespaces: {:?}", self.namespaces);
        if !self.has_usable_namespace() {
            return Ok(());
        }

        let io_queue = self.io_queue.lock();
        self.admin_queue
            .lock()
            .submit_command(NvmeCommand::create_completion_queue(
                io_queue.completion_queue_addr(),
                io_queue.queue_group_id as usize,
                io_queue.queue_size - 1,
            ))?;

        self.admin_queue
            .lock()
            .submit_command(NvmeCommand::create_submission_queue(
                io_queue.submission_queue_addr(),
                io_queue.queue_group_id as usize,
                io_queue.queue_size - 1,
            ))?;

        Ok(())
    }

    fn map_address(&self) {
        let page_start = Page::containing_address(VirtualAddress::new(self.base_address.as_u64()));
        let page_end = Page::containing_address(VirtualAddress::new(
            self.base_address.as_u64() + Self::QUEUE_ADDR_OFFSET as u64,
        ));

        for page in Page::range_inclusive(page_start, page_end) {
            let frame = PhysicalFrame::containing_address(PhysicalAddress::new(
                page.start_address.as_u64(),
            ));
            MEMORY_MAPPER
                .get()
                .unwrap()
                .map_mmio_page(page, frame)
                .expect("NVMe BAR must be uncached device memory");
        }
    }

    fn identify_controller(&self) -> Result<IdentifyController, u16> {
        let data = Dma::<UnsafeCell<[u8; 4096]>>::zeroed();
        let data_ptr = data.addr().as_u64();
        let command = NvmeCommand::create_identify(
            data_ptr,
            NvmeAdminIdentifyCns::IdentifyController,
            Default::default(),
        );
        self.admin_queue.lock().submit_command(command)?;
        // SAFETY: a successful completion returned ownership of all 4096 bytes. The
        // copied prefix fits and IdentifyController contains only integer/byte fields.
        Ok(unsafe { data.get().cast::<IdentifyController>().read_unaligned() })
    }

    fn identify_namespaces(&self) -> Result<Vec<Namespace>, u16> {
        // Identify returns a 4 KiB list of active IDs greater than the supplied ID.
        let ids = Dma::<UnsafeCell<[u32; 1024]>>::zeroed();
        let mut namespaces = Vec::new();
        let mut last_id = 0;
        loop {
            let command = NvmeCommand::create_identify(
                ids.addr().as_u64(),
                NvmeAdminIdentifyCns::IdentifyNamespacesId,
                last_id,
            );
            self.admin_queue.lock().submit_command(command)?;

            // SAFETY: completion retired the DMA write. Copying avoids references across the next command.
            let completed_ids = unsafe { ids.get().read() };
            for id in completed_ids {
                if id == 0 {
                    return Ok(namespaces);
                }
                let data = Dma::<UnsafeCell<[u8; 4096]>>::zeroed();
                trace!("Identifying namespace: {}", id);
                let command = NvmeCommand::create_identify(
                    data.addr().as_u64(),
                    NvmeAdminIdentifyCns::IdentifyNamespace,
                    id,
                );
                self.admin_queue.lock().submit_command(command)?;
                // SAFETY: the initialized completed buffer is large enough. Namespace fields are integers/arrays.
                let namespace = unsafe { data.get().cast::<IdentifyNamespace>().read_unaligned() };
                namespaces.push(namespace.as_namespace(id));
                last_id = id;
            }
        }
    }

    fn read32(&self, offset: usize) -> u32 {
        // SAFETY: private call sites use aligned fixed offsets inside the live mapped BAR.
        unsafe { ((self.base_address.as_u64() as usize + offset) as *const u32).read_volatile() }
    }
    fn read64(&self, offset: usize) -> u64 {
        // SAFETY: private call sites use aligned fixed offsets inside the live mapped BAR.
        unsafe { ((self.base_address.as_u64() as usize + offset) as *const u64).read_volatile() }
    }
    fn write32(&self, offset: usize, value: u32) {
        // SAFETY: boot/reset serialize these fixed width configuration register writes.
        unsafe {
            ((self.base_address.as_u64() as usize + offset) as *mut u32).write_volatile(value);
        }
    }
    fn write64(&self, offset: usize, value: u64) {
        // SAFETY: boot serializes these aligned queue address register writes.
        unsafe {
            ((self.base_address.as_u64() as usize + offset) as *mut u64).write_volatile(value);
        }
    }
    fn set_enabled(&self, enabled: bool) {
        self.write32(0x14, (self.read32(0x14) & !1) | u32::from(enabled));
        // Keep queue memory live until the controller acknowledges the transition.
        while (self.read32(0x1c) & 1 != 0) != enabled {
            spin_loop();
        }
    }
}

// SAFETY: reads copy from the DMA buffer only after a successful completion.
impl BlockDevice for NvmeController {
    fn read_block(&self, sector: usize, buffer: &mut [u8]) -> Result<(), BlockError> {
        if buffer.is_empty() {
            return Ok(());
        }
        if buffer.len() > MAX_TRANSFER
            || (sector as u64)
                .checked_add(buffer.len().div_ceil(SECTOR_SIZE) as u64)
                .is_none_or(|end| end > self.sector_count())
        {
            return Err(BlockError);
        }
        let data =
            Dma::<UnsafeCell<u8>>::zeroed_slice(buffer.len().div_ceil(SECTOR_SIZE) * SECTOR_SIZE);

        let command = NvmeCommand::create_read_write(
            NvmeIoCommand::Read,
            sector,
            data.addr(),
            buffer.len(),
            self.namespaces[0].block_size,
            self.namespaces[0].namespace_id,
        );
        self.io_queue
            .lock()
            .submit_command(command)
            .map_err(|_| BlockError)?;

        // SAFETY: successful completion returned the initialized DMA payload to the CPU.
        // The checked destination is a separate allocation and fits the payload buffer.
        unsafe {
            ptr::copy_nonoverlapping(
                data.as_ptr().cast::<u8>(),
                buffer.as_mut_ptr(),
                buffer.len(),
            );
        }
        Ok(())
    }

    fn write_block(&self, sector: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if !buffer.len().is_multiple_of(SECTOR_SIZE)
            || buffer.len() > MAX_TRANSFER
            || (sector as u64)
                .checked_add((buffer.len() / SECTOR_SIZE) as u64)
                .is_none_or(|end| end > self.sector_count())
        {
            return Err(BlockError);
        }
        if buffer.is_empty() {
            return Ok(());
        }

        let mut data = Dma::<u8>::zeroed_slice(buffer.len());
        data.copy_from_slice(buffer);

        let command = NvmeCommand::create_read_write(
            NvmeIoCommand::Write,
            sector,
            data.addr(),
            buffer.len(),
            self.namespaces[0].block_size,
            self.namespaces[0].namespace_id,
        );
        self.io_queue
            .lock()
            .submit_command(command)
            .map_err(|_| BlockError)?;
        Ok(())
    }

    fn sector_count(&self) -> u64 {
        self.namespaces[0].blocks as u64
    }
}

impl Drop for NvmeController {
    fn drop(&mut self) {
        if self.mapped {
            self.set_enabled(false);
        }
    }
}

#[derive(Debug)]
pub struct Namespace {
    pub namespace_id: u32,
    pub blocks: usize,
    pub block_size: usize,
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_drivers::nvme::TEST_CONTROLLERS;

    #[test(target = "qemu")]
    fn test_nvme_boot_controller() {
        let Some(controller) = TEST_CONTROLLERS.get().and_then(|c| c.first()) else {
            return;
        };
        let identify = controller.identify_controller().unwrap();
        assert_eq!(identify.serial_number.as_str(), "feedcafe");
        assert_eq!(controller.namespaces.len(), 1);
        let queue = controller.io_queue.lock();
        assert_eq!(queue.queue_group_id, 1);
        assert_eq!(queue.queue_size, NVME_QUEUE_SIZE);
    }
}
