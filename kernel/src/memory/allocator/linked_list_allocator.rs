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
        let mut node = Block::new(size);
        node.next = self.head.next.take();

        let node_ptr = addr as *mut Block;
        node_ptr.write(node);

        self.head.next = Some(&mut *node_ptr);
    }

    unsafe fn alloc_block(&mut self, layout: Layout) -> Option<usize> {
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
            if new_block_size > 0 {
                self.add_free_region(end_addr, new_block_size);
            }

            return Some(start_addr);
        }

        None
    }

    unsafe fn dealloc_block(&mut self, ptr: usize, layout: Layout) {
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
