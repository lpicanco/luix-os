use limine::memory_map::{Entry, EntryType};
use limine::request::{HhdmRequest, MemoryMapRequest};
use spin::Once;

use crate::arch::memory::mapper::MemoryMapper;
use crate::memory::address::VirtualAddress;
use crate::{println, trace};

pub mod address;
pub mod allocator;
pub mod frame;

static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
pub static MEMORY_MAPPER: Once<MemoryMapper> = Once::new();

pub fn init() {
    let pm_offset = HHDM_REQUEST
        .get_response()
        .expect("Failed to get PM offset")
        .offset();
    trace!("Physical memory offset: {:#X}", pm_offset);

    let memory_map = MEMORY_MAP_REQUEST
        .get_response()
        .expect("Failed to get memory map");

    MEMORY_MAPPER.call_once(|| MemoryMapper::new(VirtualAddress::new(pm_offset)));
    allocator::init(memory_map.entries());

    println!(
        "Memory initialized. Available memory: {:.0}MB",
        calculate_available_memory(memory_map.entries()) as f64 / 1024.0 / 1024.0
    )
}

fn calculate_available_memory(entries: &[&Entry]) -> usize {
    entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::USABLE)
        .map(|entry| entry.length as usize)
        .sum()
}

#[cfg(test)]
mod tests {
    use crate::memory::allocator::FRAME_ALLOCATOR;
    use crate::memory::MEMORY_MAPPER;

    #[test_case]
    fn test_frame_allocator_initialized() {
        assert_ne!(FRAME_ALLOCATOR.lock().memory_map.len(), 0);
    }

    #[test_case]
    fn test_memory_mapper_initialized() {
        assert!(MEMORY_MAPPER.is_completed());
    }
}
