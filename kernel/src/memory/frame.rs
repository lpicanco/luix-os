use crate::memory::address::PhysicalAddress;

#[repr(C)]
pub struct PhysicalFrame {
    pub start_address: PhysicalAddress,
}

impl PhysicalFrame {
    const FRAME_SIZE: u64 = 4096;

    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            start_address: address.align_down(Self::FRAME_SIZE),
        }
    }

    pub fn containing_address_size(address: PhysicalAddress, size: u64) -> Self {
        Self {
            start_address: address.align_down(size),
        }
    }

    pub fn size() -> u64 {
        Self::FRAME_SIZE
    }
}
