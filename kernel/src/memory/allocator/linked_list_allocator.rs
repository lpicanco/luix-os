use core::alloc::{GlobalAlloc, Layout};
use core::mem;

use crate::memory::allocator::{align_up, MutexWrapper};

pub struct LinkedListAllocator {
    head: Block,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: Block::new(0),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size)
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert!(
            size >= mem::size_of::<Block>(),
            "Size should be at least the size of a block. Size: {}",
            size
        );

        let mut new_block = Block::new(size);
        let new_block_ptr = addr as *mut Block;

        // find the last block before the new block address
        let mut leftmost_block = &mut self.head;
        while let Some(ref mut block) = leftmost_block.next {
            if block.start_addr() > addr {
                break;
            }
            leftmost_block = leftmost_block.next.as_mut().unwrap();
        }

        // get the block after the new block
        let rightmost_block = leftmost_block.next.take();

        // merge the new block with the rightmost block if possible
        if leftmost_block.size > 0 && rightmost_block.is_some() {
            let rightmost_block = rightmost_block.unwrap();

            // check if end of the new block is the start of the rightmost block
            if addr + size == rightmost_block.start_addr() {
                new_block.size += rightmost_block.size;
                new_block.next = rightmost_block.next.take();
            } else {
                new_block.next = Some(rightmost_block);
            }
        } else {
            new_block.next = rightmost_block;
        }

        new_block_ptr.write(new_block);
        leftmost_block.next = Some(&mut *new_block_ptr);
    }

    pub unsafe fn alloc_block(&mut self, layout: Layout) -> Option<usize> {
        // Resize layout to be at least the size of a block
        let (size, align) = self.align_with_block_size(layout);

        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            let start_addr = align_up(block.start_addr(), align);
            let end_addr = start_addr + size;
            let block_end_addr = block.end_addr();

            // Check if the selected block is large enough. If not, continue to the next block
            if end_addr > block_end_addr {
                current = current.next.as_mut().unwrap();
                continue;
            }

            // Remove the block from the list
            current.next = block.next.take();

            // If there is any remaining space in the block, add it to the list
            let new_block_size = block_end_addr - end_addr;
            if new_block_size >= mem::size_of::<Block>() {
                self.add_free_region(end_addr, new_block_size);
            }

            return Some(start_addr);
        }

        None
    }

    pub unsafe fn dealloc_block(&mut self, ptr: usize, layout: Layout) {
        // Resize layout to be at least the size of a block
        let (size, _) = self.align_with_block_size(layout);
        self.add_free_region(ptr, size);
    }

    fn align_with_block_size(&self, layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<Block>())
            .expect("alignment failed")
            .pad_to_align();

        let size = layout.size().max(mem::size_of::<Block>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for MutexWrapper<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock()
            .alloc_block(layout)
            .expect("Could not allocate memory.") as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().dealloc_block(ptr as usize, layout);
    }
}

struct Block {
    next: Option<&'static mut Block>,
    size: usize,
}

impl Block {
    pub const fn new(size: usize) -> Self {
        Self { size, next: None }
    }

    pub fn start_addr(&self) -> usize {
        self as *const _ as usize
    }

    pub fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_init() {
        let mut allocator = LinkedListAllocator::new();
        unsafe { allocator.init(0x1000, 4096) };

        assert_eq!(allocator.head.size, 0);
        assert!(allocator.head.next.is_some());
        assert_eq!(allocator.head.next.unwrap().size, 4096);
    }

    #[test_case]
    fn test_alloc_block() {
        let mut allocator = LinkedListAllocator::new();
        unsafe {
            allocator.init(0x1000, 2048);
            let a = allocator.alloc_block(Layout::from_size_align(1024, 1).unwrap());
            let b = allocator.alloc_block(Layout::from_size_align(256, 1).unwrap());
            let c = allocator.alloc_block(Layout::from_size_align(128, 1).unwrap());
            let d = allocator.alloc_block(Layout::from_size_align(64, 1).unwrap());

            assert_eq!(a, Some(0x1000));
            assert_eq!(b, Some(0x1400));
            assert_eq!(c, Some(0x1500));
            assert_eq!(d, Some(0x1580));
        };

        let next = allocator.head.next.unwrap();
        assert_eq!(next.size, 576);
        assert!(next.next.is_none());
    }

    #[test_case]
    fn test_dealloc_block() {
        let mut allocator = LinkedListAllocator::new();
        unsafe {
            allocator.init(0x1000, 2048);
            let a = allocator
                .alloc_block(Layout::from_size_align(512, 1).unwrap())
                .unwrap();
            let b = allocator
                .alloc_block(Layout::from_size_align(256, 1).unwrap())
                .unwrap();
            let c = allocator
                .alloc_block(Layout::from_size_align(256, 1).unwrap())
                .unwrap();
            allocator.dealloc_block(b, Layout::from_size_align(256, 1).unwrap());
            allocator.dealloc_block(a, Layout::from_size_align(512, 1).unwrap());
            allocator.dealloc_block(c, Layout::from_size_align(256, 1).unwrap());
        };

        assert_eq!(allocator.head.size, 0);

        let mut next = allocator.head.next.as_mut().unwrap();
        assert_eq!(next.size, 512);
        let next = next.next.as_mut().unwrap();
        assert_eq!(next.size, 256);
        let next = next.next.as_mut().unwrap();
        assert_eq!(next.size, 1280);
        assert!(next.next.is_none());
    }
}
