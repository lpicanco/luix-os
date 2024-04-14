use limine::memory_map::{Entry, EntryType};
use spin::Mutex;

use crate::memory::allocator::linked_list_allocator::LinkedListAllocator;

mod linked_list_allocator;

const HEAP_SIZE: usize = 1024 * 1024; // 1MB

#[global_allocator]
static ALLOCATOR: MutexWrapper<LinkedListAllocator> = MutexWrapper::new(LinkedListAllocator::new());

pub fn init(entries: &[&Entry]) {
    unsafe {
        ALLOCATOR.lock().init(entries, HEAP_SIZE);
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let offset = (addr as *const usize).align_offset(align);
    addr + offset
}

fn find_max_usable_memory(entries: &[&Entry], min_size: usize) -> usize {
    entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::USABLE && entry.length >= min_size as u64)
        .max_by_key(|entry| entry.length)
        .map(|entry| entry.base as usize)
        .expect("No usable memory regions found.")
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
