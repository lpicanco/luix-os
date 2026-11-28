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

    pub(crate) unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // SAFETY: init requires an exclusive mapped heap with space and alignment for Block.
        unsafe { self.add_free_region(heap_start, heap_size) }
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
        if rightmost_block.is_some() {
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

        // merge the new block with the leftmost block if possible
        if leftmost_block.end_addr() == addr {
            leftmost_block.size += new_block.size;
            leftmost_block.next = new_block.next.take();
            return;
        }

        // SAFETY: the caller supplies exclusive free storage with Block alignment and size.
        unsafe {
            new_block_ptr.write(new_block);
        }
        // SAFETY: the aligned block was just initialized and has no other live reference.
        leftmost_block.next = Some(unsafe { &mut *new_block_ptr });
    }

    pub(crate) unsafe fn alloc_block(&mut self, layout: Layout) -> Option<usize> {
        // Resize layout to be at least the size of a block
        let (size, align) = self.align_with_block_size(layout);

        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            let block_start = block.start_addr();
            let start_addr = align_up(block_start, align)?;
            let end_addr = start_addr.checked_add(size)?;
            let block_end_addr = block.end_addr();

            // Check if the selected block is large enough. If not, continue to the next block
            if end_addr > block_end_addr {
                current = current.next.as_mut().unwrap();
                continue;
            }

            // Remove the block from the list
            current.next = block.next.take();

            let prefix = start_addr - block_start;
            if prefix >= mem::size_of::<Block>() {
                // SAFETY: this aligned prefix belongs to the removed free block and
                // does not overlap the allocation or its trailing free region.
                unsafe {
                    self.add_free_region(block_start, prefix);
                }
            }

            // If there is any remaining space in the block, add it to the list
            let new_block_size = block_end_addr - end_addr;
            if new_block_size >= mem::size_of::<Block>() {
                // SAFETY: this aligned tail belongs to the removed block and lies beyond the allocation.
                unsafe {
                    self.add_free_region(end_addr, new_block_size);
                }
            }

            return Some(start_addr);
        }

        None
    }

    pub(crate) unsafe fn dealloc_block(&mut self, ptr: usize, layout: Layout) {
        // Resize layout to be at least the size of a block
        let (size, _) = self.align_with_block_size(layout);
        // SAFETY: the caller returns a live allocation with its original layout exactly once.
        unsafe {
            self.add_free_region(ptr, size);
        }
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

// SAFETY: the mutex serializes access to disjoint heap allocations. Layouts are
// normalized consistently for allocation and release.
unsafe impl GlobalAlloc for MutexWrapper<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the lock grants exclusive access to the heap allocator's state.
        unsafe { self.lock().alloc_block(layout) }
            .map_or(core::ptr::null_mut(), |address| address as *mut u8)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc supplies the original pointer and layout. The lock serializes release.
        unsafe {
            self.lock().dealloc_block(ptr as usize, layout);
        }
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

    const HEAP_SIZE: usize = 2048;

    /// Test-owned backing memory. These tests used to init the allocator at a
    /// fixed address inside the live kernel heap, overwriting it under QEMU.
    #[repr(C, align(16))]
    struct TestHeap([u8; HEAP_SIZE]);

    impl TestHeap {
        fn new() -> Self {
            Self([0; HEAP_SIZE])
        }

        fn start(&mut self) -> usize {
            self.0.as_mut_ptr() as usize
        }
    }

    #[test]
    fn test_init() {
        let mut heap = TestHeap::new();
        let mut allocator = LinkedListAllocator::new();
        // SAFETY: the aligned test heap outlives the allocator and is exclusively owned.
        unsafe { allocator.init(heap.start(), HEAP_SIZE) };

        assert_eq!(allocator.head.size, 0);
        assert!(allocator.head.next.is_some());
        assert_eq!(allocator.head.next.unwrap().size, HEAP_SIZE);
    }

    #[test]
    fn test_alloc_block() {
        let mut heap = TestHeap::new();
        let start = heap.start();
        let mut allocator = LinkedListAllocator::new();
        // SAFETY: this aligned test heap outlives the allocator and is exclusively owned.
        unsafe {
            allocator.init(start, HEAP_SIZE);
        }
        for (size, offset) in [(1024, 0), (256, 0x400), (128, 0x500), (64, 0x580)] {
            // SAFETY: the initialized allocator retains storage owned by this test.
            let address =
                unsafe { allocator.alloc_block(Layout::from_size_align(size, 1).unwrap()) };
            assert_eq!(address, Some(start + offset));
        }
        let next = allocator.head.next.unwrap();
        assert_eq!(next.size, 576);
        assert!(next.next.is_none());
    }

    #[test]
    fn respects_large_alignment_and_rejects_oversized_allocations() {
        let mut heap = TestHeap::new();
        let mut allocator = LinkedListAllocator::new();
        // SAFETY: exclusive aligned test storage remains alive for the allocator's lifetime.
        unsafe {
            allocator.init(heap.start(), HEAP_SIZE);
        }
        let layout = Layout::from_size_align(17, 256).unwrap();
        // SAFETY: the initialized allocator exclusively owns the live test heap.
        let address = unsafe { allocator.alloc_block(layout) }.unwrap();
        assert_eq!(address % 256, 0);
        // SAFETY: this is the original allocation pointer/layout pair, released once.
        unsafe {
            allocator.dealloc_block(address, layout);
        }
        let huge = Layout::from_size_align(isize::MAX as usize - 15, 8).unwrap();
        // SAFETY: the allocator is live. An unfulfillable request must return None.
        assert_eq!(unsafe { allocator.alloc_block(huge) }, None);
    }

    #[test]
    fn test_dealloc_block() {
        let mut heap = TestHeap::new();
        let mut allocator = LinkedListAllocator::new();
        // SAFETY: the aligned test heap outlives the allocator and is exclusively owned.
        unsafe {
            allocator.init(heap.start(), HEAP_SIZE);
        }
        let mut addresses = [0usize; 3];
        let sizes = [512, 256, 256];
        for (address, size) in addresses.iter_mut().zip(sizes) {
            // SAFETY: allocator is initialized with exclusively owned live storage.
            *address = unsafe { allocator.alloc_block(Layout::from_size_align(size, 1).unwrap()) }
                .unwrap();
        }
        for index in [1, 0, 2] {
            // SAFETY: this is the original pointer/layout pair, released exactly once.
            unsafe {
                allocator.dealloc_block(
                    addresses[index],
                    Layout::from_size_align(sizes[index], 1).unwrap(),
                );
            }
        }
        assert_eq!(allocator.head.size, 0);
        let next = allocator.head.next.as_mut().unwrap();
        assert_eq!(next.size, HEAP_SIZE);
        assert!(next.next.is_none());
    }
}
