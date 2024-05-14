use limine::memory_map::{Entry, EntryType};

use crate::memory::address::PhysicalAddress;
use crate::memory::frame::{PhysicalFrame, FRAME_SIZE};

pub struct FrameAllocator {
    // TODO: Keep the iterator instead of the memory map
    pub memory_map: &'static [&'static Entry],
    pub next: usize,
}

impl FrameAllocator {
    pub fn new(entries: &'static [&'static Entry]) -> Self {
        Self {
            memory_map: entries,
            next: 0,
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysicalFrame> {
        self.allocate_frames(FRAME_SIZE)
    }

    pub fn allocate_frames(&mut self, size: usize) -> Option<PhysicalFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += size.div_ceil(FRAME_SIZE);
        frame
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysicalFrame> {
        self.memory_map
            .iter()
            .filter(|entry| entry.entry_type == EntryType::USABLE)
            .flat_map(|entry| (entry.base..entry.base + entry.length).step_by(FRAME_SIZE))
            .map(|address| PhysicalFrame::containing_address(PhysicalAddress::new(address)))
    }
}
