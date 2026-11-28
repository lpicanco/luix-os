#[cfg(target_os = "none")]
use limine::memory_map::Entry;
#[cfg(target_os = "none")]
use spin::once::Once;
use spin::Mutex;

#[cfg(target_os = "none")]
use crate::arch::memory::paging::Page;
#[cfg(target_os = "none")]
use crate::memory::address::VirtualAddress;
#[cfg(target_os = "none")]
use crate::memory::allocator::frame_allocator::FrameAllocator;
#[cfg(target_os = "none")]
use crate::memory::allocator::linked_list_allocator::LinkedListAllocator;
#[cfg(target_os = "none")]
use crate::memory::MEMORY_MAPPER;

#[cfg(target_os = "none")]
pub mod dma_allocator;
pub mod frame_allocator;
mod linked_list_allocator;

#[cfg(target_os = "none")]
const HEAP_START: usize = 0xFEED_CAFE_000;
#[cfg(target_os = "none")]
const HEAP_SIZE: usize = 1024 * 1024 * 5; // 5MB

// Only the kernel installs this allocator; host tests keep std's.
#[cfg(target_os = "none")]
#[global_allocator]
static ALLOCATOR: MutexWrapper<LinkedListAllocator> = MutexWrapper::new(LinkedListAllocator::new());

#[cfg(target_os = "none")]
pub static FRAME_ALLOCATOR: Once<Mutex<FrameAllocator>> = Once::new();

macro_rules! allocate_frame {
    () => {
        crate::memory::allocator::FRAME_ALLOCATOR
            .get()
            .unwrap()
            .lock()
            .allocate_frame()
            .expect("Out of memory")
    };
    ($size:expr) => {
        crate::memory::allocator::FRAME_ALLOCATOR
            .get()
            .unwrap()
            .lock()
            .allocate_frames($size)
            .expect("Out of memory")
    };
}

pub(crate) use allocate_frame;

#[cfg(target_os = "none")]
pub fn init(entries: &'static [&'static Entry]) {
    FRAME_ALLOCATOR.call_once(|| Mutex::new(FrameAllocator::new(entries)));
    let heap_page_start = Page::containing_address(VirtualAddress::new(HEAP_START as u64));
    let heap_page_end =
        Page::containing_address(VirtualAddress::new((HEAP_START + HEAP_SIZE - 1) as u64));
    for page in Page::range_inclusive(heap_page_start, heap_page_end) {
        let frame = allocate_frame!();
        MEMORY_MAPPER
            .get()
            .unwrap()
            .map_page(page, frame, false, true);
    }
    // SAFETY: boot initialization has mapped this exclusive, aligned heap range.
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }
}

fn align_up(addr: usize, align: usize) -> Option<usize> {
    addr.checked_add(align - 1)
        .map(|rounded| rounded & !(align - 1))
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

#[cfg(all(test, target_os = "none"))]
mod tests {
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec::Vec;

    #[test(target = "qemu")]
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
