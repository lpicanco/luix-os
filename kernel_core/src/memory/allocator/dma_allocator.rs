use alloc::boxed::Box;
use core::alloc::{AllocError, Allocator, Layout};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::ptr::NonNull;

use crate::memory::address::PhysicalAddress;
use crate::memory::allocator::FRAME_ALLOCATOR;
use crate::memory::MEMORY_MAPPER;

#[derive(Debug)]
#[repr(C)]
pub struct Dma<T: ?Sized>(Box<T, DmaAllocator>);

/// Types whose representation is valid when every bit is zero.
/// # Safety
/// Every field must accept zero bits, including through arrays and cells.
pub(crate) unsafe trait Zeroable {}
// SAFETY: zero is a valid integer value.
unsafe impl Zeroable for u8 {}
// SAFETY: zero is a valid integer value.
unsafe impl Zeroable for u32 {}
// SAFETY: each array element accepts the zero representation.
unsafe impl<T: Zeroable, const N: usize> Zeroable for [T; N] {}
// SAFETY: UnsafeCell has the representation and validity of its contents.
unsafe impl<T: Zeroable> Zeroable for UnsafeCell<T> {}

impl<T: Zeroable> Dma<T> {
    pub fn zeroed() -> Self {
        let mut buffer = Box::new_uninit_in(DmaAllocator);
        // SAFETY: the box owns space for one T and Zeroable permits zero bits in every field.
        unsafe {
            ptr::write_bytes(buffer.as_mut_ptr(), 0, 1);
        }
        // SAFETY: every byte was initialized to zero, a valid representation of T.
        Self(unsafe { buffer.assume_init() })
    }

    pub fn zeroed_slice(len: usize) -> Dma<[T]> {
        let buffer = Box::<[T], _>::new_zeroed_slice_in(len, DmaAllocator);
        // SAFETY: Zeroable guarantees that every zeroed element is initialized and valid.
        Dma(unsafe { buffer.assume_init() })
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

pub struct DmaAllocator;
// SAFETY: each allocation owns a disjoint contiguous frame run, aligned to a page and
// in the direct map for its lifetime. Deallocation returns exactly that run once.
unsafe impl Allocator for DmaAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.align() > crate::memory::frame::FRAME_SIZE {
            return Err(AllocError);
        }
        let frame = FRAME_ALLOCATOR
            .get()
            .unwrap()
            .lock()
            .allocate_frames(layout.size())
            .ok_or(AllocError)?;
        let frame_address = frame.start_address.as_u64();
        let virt_address = MEMORY_MAPPER.get().unwrap().physical_memory_offset + frame_address;

        let ptr = NonNull::new(virt_address.as_mut_ptr()).ok_or(AllocError)?;
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
    // SAFETY: Block contains only integers and byte arrays.
    unsafe impl super::Zeroable for Block {}
    #[test(target = "qemu")]
    fn test_unavailable_contiguous_dma_allocation() {
        use super::{Allocator, DmaAllocator, Layout};

        let layout = Layout::from_size_align(isize::MAX as usize, 1).unwrap();
        assert!(DmaAllocator.allocate(layout).is_err());
    }

    #[test(target = "qemu")]
    fn test_dma_allocator() {
        let block = Dma::<Block>::zeroed();
        assert_eq!(block.field1, 0);
        assert_eq!(block.field2, 0);
        assert_eq!(block.field3.iter().all(|&x| x == 0), true);
    }

    #[test(target = "qemu")]
    fn test_dma_dealloc() {
        let block = Dma::<Block>::zeroed();
        let addr = block.addr();
        drop(block);
        assert_eq!(addr, Dma::<Block>::zeroed().addr());
    }

    #[test(target = "qemu")]
    fn test_dma_dealloc_heavy() {
        let block = Dma::<[u8; 0x101_000]>::zeroed();
        let addr = block.addr();
        drop(block);

        let block = Dma::<[u8; 0x100_000]>::zeroed();
        let block2 = Dma::<[u8; 0x1000]>::zeroed();
        assert_eq!(addr.as_u64(), block2.addr().as_u64());
    }

    #[test(target = "qemu")]
    fn test_dma_allocator_zeroed_slice() {
        let block = Dma::<Block>::zeroed_slice(5);
        assert_eq!(block[0].field1, 0);
        assert_eq!(block[3].field2, 0);
        assert_eq!(block[4].field3.iter().all(|&x| x == 0), true);
    }

    #[test(target = "qemu")]
    fn test_dma_allocator_zeroed_slice_dealloc() {
        // Allocate one block of 8 elements
        let addr = {
            let block = Dma::<Block>::zeroed_slice(8);
            block.addr()
        };

        {
            // after deallocation, allocate a blocks of 5 and 3 elements
            let block = Dma::<Block>::zeroed_slice(5);
            let block2 = Dma::<Block>::zeroed_slice(3);
            // check if the address is the same as the first allocation
            assert_eq!(addr, block2.addr());
        }

        // after deallocation, allocate one block of 8 elements again
        let block = Dma::<Block>::zeroed_slice(8);
        // check if the address is the same as the first allocation
        assert_eq!(addr, block.addr());
    }
}
