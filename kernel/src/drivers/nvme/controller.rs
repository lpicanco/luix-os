use alloc::vec::Vec;
use core::fmt;
use core::mem::MaybeUninit;

use crate::arch::instructions;
use crate::arch::memory::paging::Page;
use crate::bits::Bits;
use crate::drivers::nvme::command::{
    IdentifyController, IdentifyNamespace, NvmeAdminIdentifyCns, NvmeCommand, NvmeIoCommand,
};
use crate::drivers::nvme::queue::QueueGroup;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::allocator::dma_allocator::Dma;
use crate::memory::frame::PhysicalFrame;
use crate::memory::MEMORY_MAPPER;
use crate::trace;

/// NVMe Controller
/// Specification: https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2.0d-2024.01.11-Ratified.pdf
pub(crate) struct NvmeController {
    base_address: PhysicalAddress,
    admin_queue: QueueGroup,
    io_queue: QueueGroup,
    pub namespaces: Vec<Namespace>,
    pub identify_controller: Option<IdentifyController>,
}

impl NvmeController {
    const QUEUE_ADDR_OFFSET: usize = 0x1000; // Offset from the base address where the queues are located
    pub fn new(base_address: PhysicalAddress) -> Self {
        let queue_size = 5;
        let queue_base_address = base_address.as_u64() as usize + Self::QUEUE_ADDR_OFFSET;

        NvmeController {
            base_address,
            admin_queue: QueueGroup::new(queue_size, queue_base_address),
            io_queue: QueueGroup::new(queue_size, queue_base_address),
            namespaces: Vec::new(),
            identify_controller: None,
        }
    }

    pub(crate) fn init(&mut self) {
        self.map_address();

        let regs = self.get_registers();
        // reset controller
        regs.disable_controller();

        regs.admin_queue_attributes =
            ((self.admin_queue.queue_size - 1) << 16 | (self.admin_queue.queue_size - 1)) as u32;
        regs.admin_submission_queue_base_address =
            self.admin_queue.submission_queue_addr().as_u64();
        regs.admin_completion_queue_base_address =
            self.admin_queue.completion_queue_addr().as_u64();

        regs.controller_configuration
            .set_controller_command_set(regs.capabilities.command_sets_supported());

        regs.controller_configuration.set_arbitration_mechanism();
        regs.controller_configuration
            .set_io_submission_queue_size(6);
        regs.controller_configuration
            .set_io_completion_queue_size(4);

        regs.enable_controller();

        let identify_controller = self.identify_controller();
        trace!("Identify controller: {:?}", identify_controller);
        self.identify_controller = Some(identify_controller);

        self.namespaces = self.identify_namespaces();
        trace!("Namespaces: {:?}", self.namespaces);

        self.admin_queue
            .submit_command(NvmeCommand::create_completion_queue(
                self.io_queue.completion_queue_addr(),
                self.io_queue.queue_group_id as usize,
                self.io_queue.queue_size, //TODO: check this
            ));

        self.admin_queue
            .submit_command(NvmeCommand::create_submission_queue(
                self.io_queue.submission_queue_addr(),
                self.io_queue.queue_group_id as usize,
                self.io_queue.queue_size,
            ));

        trace!("NVMe registers: {:?}", self.get_registers());
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

    fn identify_controller(&mut self) -> IdentifyController {
        let data = Dma::<IdentifyController>::zeroed();
        let data_ptr = data.addr().as_u64();
        let command = NvmeCommand::create_identify(
            data_ptr,
            NvmeAdminIdentifyCns::IdentifyController,
            Default::default(),
        );
        self.admin_queue.submit_command(command);
        data.clone()
    }

    fn identify_namespaces(&mut self) -> Vec<Namespace> {
        let num_namespaces = self
            .identify_controller
            .as_ref()
            .unwrap()
            .number_of_namespaces as usize;
        let namespaces_id = Dma::<u32>::new_uninit_slice(num_namespaces);

        let command = NvmeCommand::create_identify(
            namespaces_id.addr().as_u64(),
            NvmeAdminIdentifyCns::IdentifyNamespacesId,
            Default::default(),
        );
        self.admin_queue.submit_command(command);

        namespaces_id
            .assume_init()
            .iter()
            .filter(|ns_id| **ns_id > 0)
            .map(|ns_id| {
                let data = Dma::<IdentifyNamespace>::zeroed();
                trace!("Identifying namespace: {}", ns_id);
                let command = NvmeCommand::create_identify(
                    data.addr().as_u64(),
                    NvmeAdminIdentifyCns::IdentifyNamespace,
                    *ns_id,
                );
                self.admin_queue.submit_command(command);
                data.as_namespace(*ns_id)
            })
            .collect()
    }

    pub fn read_block(&mut self, sector: usize, buffer: &mut [MaybeUninit<u8>]) -> usize {
        let data = Dma::<u8>::new_uninit_slice(buffer.len());

        let command = NvmeCommand::create_read_write(
            NvmeIoCommand::Read,
            sector,
            data.addr(),
            buffer.len(),
            self.namespaces[0].block_size,
            self.namespaces[0].namespace_id,
        );
        self.io_queue.submit_command(command);

        buffer.copy_from_slice(&data);
        buffer.len()
    }

    pub fn write_block(&mut self, sector: usize, buffer: &[u8]) -> usize {
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
        self.io_queue.submit_command(command);
        buffer.len()
    }

    fn get_registers(&self) -> &mut NvmeRegisters {
        unsafe { &mut *self.base_address.as_mut_ptr() }
    }
}

#[repr(transparent)]
pub struct ControllerStatus(u32);
impl ControllerStatus {
    pub(crate) fn is_ready(&self) -> bool {
        self.0.get_bit(0)
    }

    pub(crate) fn controller_fatal_status(&self) -> bool {
        self.0.get_bit(1)
    }

    fn shutdown_status(&self) -> u8 {
        self.0.get_bits(2..4) as u8
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
        self.0.get_bits(0..16) as u16
    }

    fn contiguous_queues_required(&self) -> bool {
        self.0.get_bit(16)
    }

    fn doorbell_stride(&self) -> u16 {
        self.0.get_bits(32..36) as u16
    }

    pub fn command_sets_supported(&self) -> CommandSetsSupported {
        let sets = self.0.get_bits(37..45) as u8;

        if sets.get_bit(0) {
            return CommandSetsSupported::NVM;
        }
        if sets.get_bit(6) {
            return CommandSetsSupported::IO;
        }
        if sets.get_bit(7) {
            return CommandSetsSupported::Admin;
        }
        panic!("Unknown command set: {}", sets);
    }

    fn memory_page_min_size(&self) -> u8 {
        self.0.get_bits(48..52) as u8
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
        self.0.get_bits(16..24) as u8
    }

    pub fn minor(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn tertiary(&self) -> u8 {
        self.0.get_bits(0..8) as u8
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
        self.0.get_bit(0)
    }

    pub(crate) fn set_enabled(&mut self, flag: bool) {
        self.0.set_bit(0, flag);
    }

    pub(crate) fn set_controller_command_set(&mut self, sets: CommandSetsSupported) {
        assert!(!self.is_enabled());
        self.0.set_bits(4..7, sets as u32);
    }

    pub fn set_arbitration_mechanism(&mut self) {
        let round_robin = 0b00u32;
        self.0.set_bits(11..14, round_robin);
    }

    pub(crate) fn set_io_submission_queue_size(&mut self, size: u32) {
        self.0.set_bits(16..20, size);
    }
    pub(crate) fn set_io_completion_queue_size(&mut self, size: u32) {
        self.0.set_bits(20..24, size);
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
    use alloc::boxed::Box;
    use alloc::format;
    use alloc::string::String;
    use core::mem;
    use crate::drivers::nvme::NVME_CONTROLLER;

    use super::*;

    #[test_case]
    fn test_identity_controller() {
        let mut controller = NVME_CONTROLLER.get().unwrap().lock();
        let identify = controller.identify_controller();

        assert_eq!(identify.serial_number.as_str(), "feedcafe");
    }

    #[test_case]
    fn test_namespace_identified() {
        let mut controller = NVME_CONTROLLER.get().unwrap().lock();
        let namespaces = &controller.namespaces;

        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].size, 44_040_192); // 40MB
        assert_eq!(namespaces[0].block_size, 512);
        assert_eq!(namespaces[0].blocks, 86016);
    }

    #[test_case]
    fn test_io_queues_created() {
        let mut controller = NVME_CONTROLLER.get().unwrap().lock();
        assert_eq!(controller.io_queue.queue_group_id, 1);
        assert_eq!(controller.io_queue.queue_size, 5);
    }

    #[test_case]
    fn test_read_block() {
        struct GptHeader {
            signature: [u8; 8],
            revision: [u8; 4],
        }

        let mut buffer = Box::<GptHeader>::new_uninit();
        let mut controller = NVME_CONTROLLER.get().unwrap().lock();
        let read = controller.read_block(1, buffer.as_bytes_mut());
        let block = unsafe { buffer.assume_init() };

        assert_eq!(block.signature, *b"EFI PART");
        assert_eq!(block.revision, *b"\x00\x00\x01\x00");
        assert_eq!(read, 12);
    }

    #[test_case]
    fn test_write_block() {
        struct PartitionTable {
            type_guid: [u8; 16],
            dummy: [u8; 40],
            partition_name: [u16; 36],
        }

        // Read the partition table
        let mut buffer = Box::<PartitionTable>::new_uninit();
        let read = NVME_CONTROLLER
            .get()
            .unwrap()
            .lock()
            .read_block(2, buffer.as_bytes_mut());
        let block = unsafe { buffer.assume_init() };

        let partition_name = String::from_utf16_lossy(&block.partition_name);
        assert_eq!(partition_name.trim_end_matches('\0'), "luix-os-test");

        // Write a partition table with a different name
        let new_partition_name: Vec<u16> = format!("{:\0<36}", "luix-os-test-new")
            .encode_utf16()
            .collect();
        assert_eq!(new_partition_name.len(), 36);
        let new_block = PartitionTable {
            type_guid: block.type_guid,
            dummy: block.dummy,
            partition_name: new_partition_name.as_slice().try_into().unwrap(),
        };
        let data = unsafe {
            core::slice::from_raw_parts(
                &new_block as *const _ as *const u8,
                mem::size_of::<PartitionTable>(),
            )
        };
        let wrote = NVME_CONTROLLER.get().unwrap().lock().write_block(2, data);

        // Read the partition table again
        let mut buffer_after_write = Box::<PartitionTable>::new_uninit();
        let read_after_write = NVME_CONTROLLER
            .get()
            .unwrap()
            .lock()
            .read_block(2, buffer_after_write.as_bytes_mut());
        let block_after_write = unsafe { buffer_after_write.assume_init() };

        // Check if the partition name was written correctly
        let partition_name_after_write =
            String::from_utf16_lossy(&block_after_write.partition_name);
        assert_eq!(
            partition_name_after_write.trim_end_matches('\0'),
            "luix-os-test-new"
        );
        assert_eq!(read, read_after_write);
        assert_eq!(block.type_guid, block_after_write.type_guid);
        assert_eq!(block.dummy, block_after_write.dummy);
    }
}
