use alloc::vec::Vec;

use limine::memory_map::{Entry, EntryType};

use crate::memory::address::PhysicalAddress;
use crate::memory::frame::{PhysicalFrame, FRAME_SIZE};

struct UsableBlock {
    start: u64,
    size: u64,
}

pub struct FrameAllocator {
    // TODO: Keep the iterator instead of the memory map
    memory_map: &'static [&'static Entry],
    reusable_frames: Vec<PhysicalFrame>,
    next: usize,
}

impl FrameAllocator {
    pub fn new(entries: &'static [&'static Entry]) -> Self {
        let reusable_frames = Vec::new();
        Self {
            memory_map: entries,
            reusable_frames,
            next: 0,
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysicalFrame> {
        self.allocate_frames(FRAME_SIZE)
    }

    pub fn allocate_frames(&mut self, size: usize) -> Option<PhysicalFrame> {
        let frame_count = size.div_ceil(FRAME_SIZE);
        if frame_count == 0 {
            return None;
        }
        // Individually freed process pages can leave gaps between reusable frames.
        let last_offset = ((frame_count - 1) * FRAME_SIZE) as u64;
        if let Some(index) = self
            .reusable_frames
            .windows(frame_count)
            .rposition(|frames| {
                frames[0].start_address + last_offset == frames[frame_count - 1].start_address
            })
        {
            let frame = self.reusable_frames[index];
            self.reusable_frames.drain(index..index + frame_count);
            return Some(frame);
        }

        let frame = self.usable_frames().nth(self.next)?;
        self.next += frame_count;
        Some(frame)
    }

    pub fn deallocate_frames(&mut self, start_address: PhysicalAddress, size: usize) {
        let end_address = start_address + (size as u64 - 1);

        let start_frame = PhysicalFrame::containing_address(start_address);
        let end_frame = PhysicalFrame::containing_address(end_address);

        PhysicalFrame::range_inclusive(start_frame, end_frame).for_each(|frame| {
            self.reusable_frames.push(frame);
        });
        self.reusable_frames.sort();
    }

    #[cfg(test)]
    pub(crate) fn allocated_frame_count(&self) -> usize {
        self.next - self.reusable_frames.len()
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame> {
        self.memory_map
            .iter()
            .filter(|entry| entry.entry_type == EntryType::USABLE)
            .flat_map(|entry| (entry.base..entry.base + entry.length).step_by(FRAME_SIZE))
            .map(|address| PhysicalFrame::containing_address(PhysicalAddress::new(address)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reuse_requires_contiguous_frames() {
        let mut allocator = FrameAllocator::new(&[]);
        let start = PhysicalAddress::new(FRAME_SIZE as u64);
        allocator.deallocate_frames(start, FRAME_SIZE);
        allocator.deallocate_frames(start + (2 * FRAME_SIZE) as u64, FRAME_SIZE);
        assert_eq!(allocator.allocate_frames(2 * FRAME_SIZE), None);

        allocator.deallocate_frames(start + FRAME_SIZE as u64, FRAME_SIZE);
        assert_eq!(
            allocator.allocate_frames(3 * FRAME_SIZE),
            Some(PhysicalFrame::containing_address(start))
        );
    }
}
