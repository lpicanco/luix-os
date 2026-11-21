use alloc::vec::Vec;
use core::fmt;
use core::mem::MaybeUninit;

use spin::Mutex;

use crate::arch::instructions;
use crate::arch::memory::paging::Page;
use crate::bits::Bits;
use crate::drivers::nvme::command::{
    IdentifyController, IdentifyNamespace, NvmeAdminIdentifyCns, NvmeCommand, NvmeIoCommand,
};
use crate::drivers::nvme::queue::QueueGroup;
use crate::drivers::{BlockDevice, BlockError};
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
            admin_queue: Mutex::new(QueueGroup::new(0, queue_size, queue_base_address)),
            io_queue: Mutex::new(QueueGroup::new(1, queue_size, queue_base_address)),
            namespaces: Vec::new(),
            identify_controller: None,
        }
    }

    pub(crate) fn init(&mut self) -> Result<(), u16> {
        self.map_address();

        let regs = self.get_registers();
        // reset controller
        regs.disable_controller();

        let admin_queue = self.admin_queue.lock();
        regs.admin_queue_attributes =
            ((admin_queue.queue_size - 1) << 16 | (admin_queue.queue_size - 1)) as u32;
        regs.admin_submission_queue_base_address = admin_queue.submission_queue_addr().as_u64();
        regs.admin_completion_queue_base_address = admin_queue.completion_queue_addr().as_u64();

        regs.controller_configuration
            .set_controller_command_set(regs.capabilities.command_sets_supported());

        regs.controller_configuration.set_arbitration_mechanism();
        regs.controller_configuration
            .set_io_submission_queue_size(6);
        regs.controller_configuration
            .set_io_completion_queue_size(4);
        drop(admin_queue);

        regs.enable_controller();

        let result = self.initialize_queues_and_namespaces();
        if result.is_err() || self.namespaces.is_empty() {
            // Stop DMA before the caller releases this controller's queues.
            self.get_registers().disable_controller();
        }
        result
    }

    fn initialize_queues_and_namespaces(&mut self) -> Result<(), u16> {
        let identify_controller = self.identify_controller()?;
        trace!("Identify controller: {:?}", identify_controller);
        self.identify_controller = Some(identify_controller);

        self.namespaces = self.identify_namespaces()?;
        trace!("Namespaces: {:?}", self.namespaces);
        if self.namespaces.is_empty() {
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

        trace!("NVMe registers: {:?}", self.get_registers());
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
            unsafe {
                MEMORY_MAPPER
                    .get_unchecked()
                    .map_page(page, frame, false, true)
            }
        }
    }

    fn identify_controller(&self) -> Result<IdentifyController, u16> {
        let data = Dma::<IdentifyController>::zeroed();
        let data_ptr = data.addr().as_u64();
        let command = NvmeCommand::create_identify(
            data_ptr,
            NvmeAdminIdentifyCns::IdentifyController,
            Default::default(),
        );
        self.admin_queue.lock().submit_command(command)?;
        Ok(data.clone())
    }

    fn identify_namespaces(&self) -> Result<Vec<Namespace>, u16> {
        // Identify returns a 4 KiB list of active IDs greater than the supplied ID.
        let ids = Dma::<[u32; 1024]>::zeroed();
        let mut namespaces = Vec::new();
        let mut last_id = 0;
        loop {
            let command = NvmeCommand::create_identify(
                ids.addr().as_u64(),
                NvmeAdminIdentifyCns::IdentifyNamespacesId,
                last_id,
            );
            self.admin_queue.lock().submit_command(command)?;

            for &id in ids.iter() {
                if id == 0 {
                    return Ok(namespaces);
                }
                let data = Dma::<IdentifyNamespace>::zeroed();
                trace!("Identifying namespace: {}", id);
                let command = NvmeCommand::create_identify(
                    data.addr().as_u64(),
                    NvmeAdminIdentifyCns::IdentifyNamespace,
                    id,
                );
                self.admin_queue.lock().submit_command(command)?;
                namespaces.push(data.as_namespace(id));
                last_id = id;
            }
        }
    }

    // The registers are device memory.
    // TODO: replace with volatile accessors.
    #[allow(clippy::mut_from_ref)]
    fn get_registers(&self) -> &mut NvmeRegisters {
        unsafe { &mut *self.base_address.as_mut_ptr() }
    }
}

// SAFETY: reads copy from the DMA buffer only after a successful completion.
unsafe impl BlockDevice for NvmeController {
    fn read_block(&self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> Result<(), BlockError> {
        if buffer.is_empty() {
            return Ok(());
        }
        let data = Dma::<u8>::new_uninit_slice(buffer.len());

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

        buffer.copy_from_slice(&data);
        Ok(())
    }

    fn write_block(&self, sector: usize, buffer: &[u8]) -> Result<(), BlockError> {
        if buffer.is_empty() {
            return Ok(());
        }
        assert!(!self.namespaces.is_empty(), "No namespaces found");

        let mut data = Dma::<u8>::new_uninit_slice(buffer.len()).assume_init();
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
}

#[repr(transparent)]
pub struct ControllerStatus(u32);
impl ControllerStatus {
    pub(crate) fn is_ready(&self) -> bool {
        unsafe { (&raw const self.0).read_volatile() }.read_bit(0)
    }

    pub(crate) fn controller_fatal_status(&self) -> bool {
        self.0.read_bit(1)
    }

    fn shutdown_status(&self) -> u8 {
        self.0.read_bits(2..4) as u8
    }
}

impl fmt::Debug for ControllerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControllerStatus")
            .field("is_ready", &self.is_ready())
            .field("controller_fatal_status", &self.controller_fatal_status())
            .field("shutdown_status", &self.shutdown_status())
            .finish()
    }
}

#[derive(Debug, Clone)]
#[repr(u32)]
pub enum CommandSetsSupported {
    NVM = 0b000,
    IO,
    Admin,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Capabilities(u64);
impl Capabilities {
    pub fn max_queue_entries(&self) -> u16 {
        self.0.read_bits(0..16) as u16
    }

    fn contiguous_queues_required(&self) -> bool {
        self.0.read_bit(16)
    }

    fn doorbell_stride(&self) -> u16 {
        self.0.read_bits(32..36) as u16
    }

    pub fn command_sets_supported(&self) -> CommandSetsSupported {
        let sets = self.0.read_bits(37..45) as u8;

        if sets.read_bit(0) {
            return CommandSetsSupported::NVM;
        }
        if sets.read_bit(6) {
            return CommandSetsSupported::IO;
        }
        if sets.read_bit(7) {
            return CommandSetsSupported::Admin;
        }
        panic!("Unknown command set: {}", sets);
    }

    fn memory_page_min_size(&self) -> u8 {
        self.0.read_bits(48..52) as u8
    }
}
impl fmt::Debug for Capabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Capabilities")
            .field("max_queue_entries", &self.max_queue_entries())
            .field(
                "contiguous_queues_required",
                &self.contiguous_queues_required(),
            )
            .field("doorbell_stride", &self.doorbell_stride())
            .field("command_sets_supported", &self.command_sets_supported())
            .field("memory_page_min_size", &self.memory_page_min_size())
            .finish()
    }
}

#[derive(Clone)]
#[repr(transparent)]
struct Version(u32);

impl Version {
    pub fn major(&self) -> u8 {
        self.0.read_bits(16..24) as u8
    }

    pub fn minor(&self) -> u8 {
        self.0.read_bits(8..16) as u8
    }

    pub fn tertiary(&self) -> u8 {
        self.0.read_bits(0..8) as u8
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major(), self.minor(), self.tertiary())
    }
}

#[repr(transparent)]
pub struct ControllerConfiguration(u32);

impl ControllerConfiguration {
    pub(crate) fn is_enabled(&self) -> bool {
        self.0.read_bit(0)
    }

    pub(crate) fn set_enabled(&mut self, flag: bool) {
        let register = &raw mut self.0;
        unsafe {
            let mut value = register.read_volatile();
            value.write_bit(0, flag);
            register.write_volatile(value);
        }
    }

    pub(crate) fn set_controller_command_set(&mut self, sets: CommandSetsSupported) {
        assert!(!self.is_enabled());
        self.0.write_bits(4..7, sets as u32);
    }

    pub fn set_arbitration_mechanism(&mut self) {
        let round_robin = 0b00u32;
        self.0.write_bits(11..14, round_robin);
    }

    pub(crate) fn set_io_submission_queue_size(&mut self, size: u32) {
        self.0.write_bits(16..20, size);
    }
    pub(crate) fn set_io_completion_queue_size(&mut self, size: u32) {
        self.0.write_bits(20..24, size);
    }
}

impl fmt::Debug for ControllerConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControllerConfiguration")
            .field("is_enabled", &self.is_enabled())
            .finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub(crate) struct NvmeRegisters {
    pub capabilities: Capabilities,
    version: Version,
    interrupt_mask_set: u32,
    interrupt_mask_clear: u32,
    pub controller_configuration: ControllerConfiguration,
    reserved1: u32,
    pub controller_status: ControllerStatus,
    nvm_subsystem_reset: u32,
    pub admin_queue_attributes: u32,
    pub admin_submission_queue_base_address: u64,
    pub admin_completion_queue_base_address: u64,
}

impl NvmeRegisters {
    pub(crate) fn disable_controller(&mut self) {
        self.controller_configuration.set_enabled(false);

        // Wait for the controller to become ready
        while self.controller_status.is_ready() {
            instructions::halt();
        }
    }

    pub(crate) fn enable_controller(&mut self) {
        self.controller_configuration.set_enabled(true);

        // Wait for the controller to become ready
        while !self.controller_status.is_ready() {
            instructions::halt();
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
    use crate::drivers::fs::test_boot_device;

    #[test(target = "qemu")]
    fn test_nvme_boot_controller() {
        let device = test_boot_device();
        let Some(controller) =
            (device.as_ref() as &dyn core::any::Any).downcast_ref::<NvmeController>()
        else {
            return;
        };
        let identify = controller.identify_controller().unwrap();
        assert_eq!(identify.serial_number.as_str(), "feedcafe");
        assert_eq!(controller.namespaces.len(), 1);
        let namespace = &controller.namespaces[0];
        assert_eq!(namespace.size, 44_040_192);
        assert_eq!(namespace.block_size, 512);
        assert_eq!(namespace.blocks, 86016);
        let queue = controller.io_queue.lock();
        assert_eq!(queue.queue_group_id, 1);
        assert_eq!(queue.queue_size, NVME_QUEUE_SIZE);
    }
}
