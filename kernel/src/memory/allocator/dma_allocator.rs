use alloc::boxed::Box;
use core::alloc::{AllocError, Allocator, Layout};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::ptr::NonNull;

use crate::allocate_frame;
use crate::memory::address::PhysicalAddress;
use crate::memory::allocator::FRAME_ALLOCATOR;
use crate::memory::MEMORY_MAPPER;

#[derive(Debug)]
#[repr(C)]
pub struct Dma<T: ?Sized>(Box<T, DmaAllocator>);

impl<T> Dma<T> {
    pub fn zeroed() -> Self {
        let mut buffer = Box::new_uninit_in(DmaAllocator);
        unsafe {
            ptr::write_bytes(buffer.as_mut_ptr(), 0, 1);
            Self(buffer.assume_init())
        }
    }

    pub fn new_zeroed_slice(len: usize) -> Dma<[MaybeUninit<T>]> {
        Dma(Box::new_zeroed_slice_in(len, DmaAllocator))
    }

    pub fn new_uninit_slice(len: usize) -> Dma<[MaybeUninit<T>]> {
        Dma(Box::new_uninit_slice_in(len, DmaAllocator))
    }
}

impl<T: ?Sized> Dma<T> {
    pub fn addr(&self) -> PhysicalAddress {
        let phys = ptr::addr_of!(*self.0).addr() as u64;
        PhysicalAddress::new(phys - MEMORY_MAPPER.get().unwrap().physical_memory_offset.as_u64())
    }
}

impl<T: ?Sized> Deref for Dma<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for Dma<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Dma<[MaybeUninit<T>]> {
    pub(crate) fn assume_init(self) -> Dma<[T]> {
        Dma(unsafe { self.0.assume_init() })
    }
}

pub struct DmaAllocator;
unsafe impl Allocator for DmaAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let frame_address = allocate_frame!(layout.size()).start_address.as_u64();
        let virt_address = MEMORY_MAPPER.get().unwrap().physical_memory_offset + frame_address;

        let ptr = unsafe { NonNull::new_unchecked(virt_address.as_mut_ptr()) };
        let size = layout.size();
        Ok(NonNull::slice_from_raw_parts(ptr, size))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let phys_addr = PhysicalAddress::new(
            ptr.as_ptr() as u64 - MEMORY_MAPPER.get().unwrap().physical_memory_offset.as_u64(),
        );
        FRAME_ALLOCATOR
            .get()
            .unwrap()
            .lock()
            .deallocate_frames(phys_addr, layout.size());
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::allocator::dma_allocator::Dma;

    struct Block {
        field1: u64,
        field2: u64,
        field3: [u8; 1024],
    }
    #[test_case]
    fn test_dma_allocator() {
        let block = Dma::<Block>::zeroed();
        assert_eq!(block.field1, 0);
        assert_eq!(block.field2, 0);
        assert_eq!(block.field3.iter().all(|&x| x == 0), true);
    }

    #[test_case]
    fn test_dma_dealloc() {
        let block = Dma::<Block>::zeroed();
        let addr = block.addr();
        drop(block);
        assert_eq!(addr, Dma::<Block>::zeroed().addr());
    }

    #[test_case]
    fn test_dma_dealloc_heavy() {
        let block = Dma::<[u8; 0x101_000]>::zeroed();
        let addr = block.addr();
        drop(block);

        let block = Dma::<[u8; 0x100_000]>::zeroed();
        let block2 = Dma::<[u8; 0x1000]>::zeroed();
        assert_eq!(addr.as_u64(), block2.addr().as_u64());
    }

    #[test_case]
    fn test_dma_allocator_zeroed_slice() {
        let block = Dma::<Block>::new_zeroed_slice(5).assume_init();
        assert_eq!(block[0].field1, 0);
        assert_eq!(block[3].field2, 0);
        assert_eq!(block[4].field3.iter().all(|&x| x == 0), true);
    }

    #[test_case]
    fn test_dma_allocator_zeroed_slice_dealloc() {
        // Allocate one block of 8 elements
        let addr = {
            let block = Dma::<Block>::new_zeroed_slice(8).assume_init();
            block.addr()
        };

        {
            // after deallocation, allocate a blocks of 5 and 3 elements
            let block = Dma::<Block>::new_zeroed_slice(5).assume_init();
            let block2 = Dma::<Block>::new_zeroed_slice(3).assume_init();
            // check if the address is the same as the first allocation
            assert_eq!(addr, block2.addr());
        }

        // after deallocation, allocate one block of 8 elements again
        let block = Dma::<Block>::new_zeroed_slice(8).assume_init();
        // check if the address is the same as the first allocation
        assert_eq!(addr, block.addr());
    }
}
