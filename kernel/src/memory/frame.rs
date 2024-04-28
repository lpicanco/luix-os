use crate::memory::address::PhysicalAddress;

pub const FRAME_SIZE: usize = 4096;
#[repr(C)]
#[derive(Clone)]
pub struct PhysicalFrame {
    pub start_address: PhysicalAddress,
}

impl PhysicalFrame {
    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            start_address: address.align_down(FRAME_SIZE as u64),
        }
    }

    pub fn size() -> u64 {
        FRAME_SIZE as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test_case]
    fn test_frame() {
        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x1000));
        assert_eq!(frame.start_address.as_u64(), 0x1000);

        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x1999));
        assert_eq!(frame.start_address.as_u64(), 0x1000);

        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x2000));
        assert_eq!(frame.start_address.as_u64(), 0x2000);

        assert_eq!(PhysicalFrame::size(), 4096);
    }
}
