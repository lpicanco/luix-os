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

    // TODO: Should be contiguous frames in some cases
    pub fn allocate_frames(&mut self, size: usize) -> Option<PhysicalFrame> {
        let frame_count = size.div_ceil(FRAME_SIZE);

        // TODO: Allow fetching partial frames
        if self.reusable_frames.len() >= frame_count {
            let frame = self
                .reusable_frames
                .split_off(self.reusable_frames.len() - frame_count);
            return Some(frame[0]);
        }

        let frame = self.usable_frames().nth(self.next);
        self.next += frame_count;
        frame
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

    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame> {
        self.memory_map
            .iter()
            .filter(|entry| entry.entry_type == EntryType::USABLE)
            .flat_map(|entry| (entry.base..entry.base + entry.length).step_by(FRAME_SIZE))
            .map(|address| PhysicalFrame::containing_address(PhysicalAddress::new(address)))
    }
}
