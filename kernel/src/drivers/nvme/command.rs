use alloc::string::String;
use core::fmt;

use crate::drivers::nvme::controller::Namespace;
use crate::memory::address::PhysicalAddress;

pub enum NvmeAdminIdentifyCns {
    IdentifyNamespace = 0x0,
    IdentifyController = 0x1,
    IdentifyNamespacesId = 0x2,
}

pub enum NvmeIoCommand {
    Write = 0x1,
    Read = 0x2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct NvmeCommand {
    pub opcode: u8,
    pub flags: u8,
    pub command_id: u16,
    pub nsid: u32,
    pub reserved0: [u32; 2],
    pub metadata_ptr: u64,
    pub dptr: [u64; 2],
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

impl NvmeCommand {
    const NVME_ADMIN_CREATE_SUBMISSION_QUEUE: u8 = 0x01;
    const NVME_ADMIN_CREATE_COMPLETION_QUEUE: u8 = 0x05;
    const NVME_ADMIN_IDENTIFY: u8 = 0x06;
    pub fn create_identify(data_ptr: u64, cns: NvmeAdminIdentifyCns, namespace_id: u32) -> Self {
        Self {
            opcode: Self::NVME_ADMIN_IDENTIFY,
            flags: 0,
            command_id: 0,
            nsid: namespace_id,
            reserved0: [0; 2],
            metadata_ptr: 0,
            dptr: [data_ptr, 0],
            cdw10: cns as u32,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        }
    }

    pub fn create_completion_queue(
        address: PhysicalAddress,
        queue_id: usize,
        queue_size: usize,
    ) -> Self {
        let cdw10: u32 = queue_id as u32 | (queue_size as u32) << 16;
        Self {
            opcode: Self::NVME_ADMIN_CREATE_COMPLETION_QUEUE,
            flags: 0,
            command_id: 0,
            nsid: 0,
            reserved0: [0; 2],
            metadata_ptr: 0,
            dptr: [address.as_u64(), 0],
            cdw10,
            cdw11: 1,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        }
    }

    pub fn create_submission_queue(
        address: PhysicalAddress,
        queue_id: usize,
        queue_size: usize,
    ) -> Self {
        Self {
            opcode: Self::NVME_ADMIN_CREATE_SUBMISSION_QUEUE,
            flags: 0,
            command_id: 0,
            nsid: 0,
            reserved0: [0; 2],
            metadata_ptr: 0,
            dptr: [address.as_u64(), Default::default()],
            cdw10: queue_id as u32 | (queue_size as u32) << 16,
            cdw11: 1u32 | (queue_id as u32) << 16,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        }
    }

    pub fn create_read_write(
        opcode: NvmeIoCommand,
        sector: usize,
        data_address: PhysicalAddress,
        data_len: usize,
        block_size: usize,
        nsid: u32,
    ) -> Self {
        let blocks = data_len.div_ceil(block_size);
        Self {
            opcode: opcode as u8,
            flags: 0,
            command_id: 0,
            nsid,
            reserved0: [0; 2],
            metadata_ptr: 0,
            dptr: [data_address.as_u64(), 0],
            cdw10: sector as u32,
            cdw11: 0,
            cdw12: (blocks - 1) as u32,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
        }
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct IdentifyController {
    pub vendor_id: u16,
    pub subsystem_vendor_id: u16,
    pub serial_number: SerialNumber,
    pub model_number: [u8; 40],
    pub firmware_version: [u8; 8],
    pub recommended_arbitration_burst: u8,
    pub ieee: [u8; 3],
    pub cmic: u8,
    pub maximum_data_transfer_size: u8,
    pub controller_id: u16,
    pub version: u32,
    pub rtd3r: u32,
    pub rtd3e: u32,
    pub oaes: u32,
    pub controller_attributes: u32,
    pub unused_100: [u8; 156],
    pub oacs: u16,
    pub acl: u8,
    pub aerl: u8,
    pub frmw: u8,
    pub lpa: u8,
    pub elpe: u8,
    pub npss: u8,
    pub avscc: u8,
    pub apsta: u8,
    pub wctemp: u16,
    pub cctemp: u16,
    pub mtfa: u16,
    pub hmpre: u32,
    pub hmmin: u32,
    pub tnvmcap: [u8; 16],
    pub unvmcap: [u8; 16],
    pub rpmbs: u32,
    pub edstt: u16,
    pub dsto: u8,
    pub fwug: u8,
    pub kas: u16,
    pub hctma: u16,
    pub mntmt: u16,
    pub mxtmt: u16,
    pub sanicap: u32,
    pub hmminds: u32,
    pub hmmaxd: u16,
    pub unused_338: [u8; 176],
    pub submission_queue_entry_size: u8,
    pub completion_queue_entry_size: u8,
    pub maximum_outstanding_commands: u16,
    pub number_of_namespaces: u32, // 516:519
}

impl fmt::Debug for IdentifyController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentifyController")
            .field("vendor_id", &self.vendor_id)
            .field("subsystem_vendor_id", &self.subsystem_vendor_id)
            .field("serial_number", &self.serial_number)
            .field(
                "model_number",
                &String::from_utf8_lossy(&self.model_number).trim(),
            )
            .field("number_of_namespaces", &self.number_of_namespaces)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct SerialNumber([u8; 20]);

impl SerialNumber {
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.0).trim() }
    }
}

impl fmt::Display for SerialNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct LbaFormat {
    pub ms: u16,
    pub ds: u8,
    pub rp: u8,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct IdentifyNamespace {
    pub nsze: u64,
    pub ncap: u64,
    pub nuse: u64,
    pub nsfeat: u8,
    pub nlbaf: u8,
    pub flbas: u8,
    pub mc: u8,
    pub dpc: u8,
    pub dps: u8,
    pub nmic: u8,
    pub rescap: u8,
    pub fpi: u8,
    pub dlfeat: u8,
    pub nawun: u16,
    pub nawupf: u16,
    pub nacwu: u16,
    pub nabsn: u16,
    pub nabo: u16,
    pub nabspf: u16,
    pub noiob: u16,
    pub nvmcap: [u8; 16],
    pub npwg: u16,
    pub npwa: u16,
    pub npdg: u16,
    pub npda: u16,
    pub nows: u16,
    pub unused_74: [u8; 53],
    pub lbaf: [LbaFormat; 16],
}

impl IdentifyNamespace {
    pub(crate) fn as_namespace(&self, namespace_id: u32) -> Namespace {
        let blocks = self.nsze as usize;
        let block_size = 1 << self.lbaf[(self.flbas & 0b11111) as usize].ds;

        Namespace {
            namespace_id,
            blocks,
            block_size,
            size: blocks * block_size,
        }
    }
}
