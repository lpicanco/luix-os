use alloc::vec::Vec;
use core::ops::Range;

use limine::memory_map::{Entry, EntryType};

use crate::memory::address::PhysicalAddress;
use crate::memory::frame::{PhysicalFrame, FRAME_SIZE};

pub struct FrameAllocator {
    entries: core::slice::Iter<'static, &'static Entry>,
    // Frame numbers, with the end excluded. Keep the current range inline so
    // single-frame allocations can bootstrap the heap without allocating.
    current: Range<u64>,
    skipped: Vec<Range<u64>>,
    reusable_frames: Vec<PhysicalFrame>,
    #[cfg(test)]
    fresh_frame_count: usize,
}

impl FrameAllocator {
    pub fn new(entries: &'static [&'static Entry]) -> Self {
        Self {
            entries: entries.iter(),
            current: 0..0,
            skipped: Vec::new(),
            reusable_frames: Vec::new(),
            #[cfg(test)]
            fresh_frame_count: 0,
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

        let first = self.allocate_fresh(frame_count as u64)?;
        #[cfg(test)]
        {
            self.fresh_frame_count += frame_count;
        }
        Some(PhysicalFrame {
            start_address: PhysicalAddress::new(first * FRAME_SIZE as u64),
        })
    }

    fn allocate_fresh(&mut self, count: u64) -> Option<u64> {
        if let Some(index) = self.skipped.iter().position(|r| r.end - r.start >= count) {
            let range = &mut self.skipped[index];
            let first = range.start;
            range.start += count;
            if range.is_empty() {
                self.skipped.swap_remove(index);
            }
            return Some(first);
        }

        while self.current.end - self.current.start < count {
            let next = self.entries.find_map(|entry| {
                if entry.entry_type != EntryType::USABLE {
                    return None;
                }
                let end = entry.base.checked_add(entry.length)?;
                let frames = entry.base.div_ceil(FRAME_SIZE as u64)..end / FRAME_SIZE as u64;
                (!frames.is_empty()).then_some(frames)
            })?;
            let previous = core::mem::replace(&mut self.current, next);
            if !previous.is_empty() {
                self.skipped.push(previous);
            }
        }
        let first = self.current.start;
        self.current.start += count;
        Some(first)
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
        self.fresh_frame_count - self.reusable_frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_allocations_stay_within_usable_ranges() {
        static ENTRIES: [&Entry; 3] = [
            &Entry {
                base: 0x1001,
                length: 0x2fff,
                entry_type: EntryType::USABLE,
            },
            &Entry {
                base: 0x4000,
                length: 0x2000,
                entry_type: EntryType::RESERVED,
            },
            &Entry {
                base: 0x6000,
                length: 0x3fff,
                entry_type: EntryType::USABLE,
            },
        ];
        let mut allocator = FrameAllocator::new(&ENTRIES);
        assert_eq!(allocator.allocate_frames(4 * FRAME_SIZE), None);
        let run = allocator.allocate_frames(2 * FRAME_SIZE + 1).unwrap();
        assert_eq!(run.start_address.as_u64(), 0x6000);
        let tail = allocator.allocate_frames(2 * FRAME_SIZE).unwrap();
        assert_eq!(tail.start_address.as_u64(), 0x2000);
        assert_eq!(allocator.allocate_frame(), None);
        assert_eq!(allocator.allocated_frame_count(), 5);
    }

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
