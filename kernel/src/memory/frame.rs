use crate::memory::address::PhysicalAddress;

pub const FRAME_SIZE: usize = 4096;
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    pub(crate) fn range_inclusive(start: PhysicalFrame, end: PhysicalFrame) -> PhysicalFrameIter {
        PhysicalFrameIter { start, end }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalFrameIter {
    start: PhysicalFrame,
    end: PhysicalFrame,
}

impl Iterator for PhysicalFrameIter {
    type Item = PhysicalFrame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.start;
            self.start = PhysicalFrame {
                start_address: frame.start_address + FRAME_SIZE as u64,
            };
            Some(frame)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test_case]
    fn test_containing_address() {
        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x1000));
        assert_eq!(frame.start_address.as_u64(), 0x1000);

        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x1999));
        assert_eq!(frame.start_address.as_u64(), 0x1000);

        let frame = PhysicalFrame::containing_address(PhysicalAddress::new(0x2000));
        assert_eq!(frame.start_address.as_u64(), 0x2000);

        assert_eq!(PhysicalFrame::size(), 4096);
    }

    #[test_case]
    fn test_range_inclusive() {
        let start = PhysicalFrame::containing_address(PhysicalAddress::new(0x1000));
        let end = PhysicalFrame::containing_address(PhysicalAddress::new(0x1FFF));
        let frames = PhysicalFrame::range_inclusive(start, end).collect::<Vec<_>>();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].start_address.as_u64(), 0x1000);

        let start = PhysicalFrame::containing_address(PhysicalAddress::new(0x1000));
        let end = PhysicalFrame::containing_address(PhysicalAddress::new(0x3200));
        let frames = PhysicalFrame::range_inclusive(start, end).collect::<Vec<_>>();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].start_address.as_u64(), 0x1000);
        assert_eq!(frames[1].start_address.as_u64(), 0x2000);
        assert_eq!(frames[2].start_address.as_u64(), 0x3000);
    }
}
