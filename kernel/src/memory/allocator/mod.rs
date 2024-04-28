use limine::memory_map::Entry;
use spin::Mutex;

use crate::arch::memory::paging::Page;
use crate::memory::address::VirtualAddress;
use crate::memory::allocator::frame_allocator::FrameAllocator;
use crate::memory::allocator::linked_list_allocator::LinkedListAllocator;
use crate::memory::MEMORY_MAPPER;

pub mod frame_allocator;
mod linked_list_allocator;

const HEAP_START: usize = 0xFEED_CAFE_000;
const HEAP_SIZE: usize = 1024 * 1024 * 2; // 2MB

#[global_allocator]
static ALLOCATOR: MutexWrapper<LinkedListAllocator> = MutexWrapper::new(LinkedListAllocator::new());

pub static FRAME_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator {
    memory_map: &[],
    next: 0,
});

pub fn init(entries: &'static [&'static Entry]) {
    unsafe {
        FRAME_ALLOCATOR.lock().memory_map = entries;

        let heap_page_start = Page::containing_address(VirtualAddress::new(HEAP_START as u64));
        let heap_page_end =
            Page::containing_address(VirtualAddress::new((HEAP_START + HEAP_SIZE - 1) as u64));
        let heap_pages = Page::range_inclusive(heap_page_start, heap_page_end);

        {
            let mut frame_allocator = FRAME_ALLOCATOR.lock();
            for page in heap_pages {
                let frame = frame_allocator.allocate_frame();
                MEMORY_MAPPER.get_unchecked().map_page(
                    page,
                    frame.unwrap(),
                    &mut frame_allocator,
                    false,
                    true,
                )
            }
        }

        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let offset = (addr as *const usize).align_offset(align);
    addr + offset
}

struct MutexWrapper<T>(Mutex<T>);
impl<T> MutexWrapper<T> {
    pub const fn new(inner: T) -> Self {
        Self(Mutex::new(inner))
    }

    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.0.lock()
    }
}

mod tests {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec::Vec;

    #[test_case]
    fn test_heap_allocator() {
        let heap_value = Box::new(42);
        assert_eq!(*heap_value, 42);

        let mut vec = Vec::new();
        for i in 0..500 {
            vec.push(i);
        }
        assert_eq!(vec.iter().sum::<u32>(), (0..500).sum());

        let rc = Rc::new(vec);
        let rc2 = Rc::clone(&rc);
        assert_eq!(rc[10], 10);
        assert_eq!(Rc::strong_count(&rc), 2);
        drop(rc2);
        assert_eq!(Rc::strong_count(&rc), 1);
    }
}
