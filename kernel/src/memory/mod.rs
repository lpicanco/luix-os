#[cfg(target_os = "none")]
use limine::memory_map::{Entry, EntryType};
#[cfg(target_os = "none")]
use limine::request::{HhdmRequest, MemoryMapRequest};
#[cfg(target_os = "none")]
use spin::Once;

#[cfg(target_os = "none")]
use crate::arch::memory::mapper::MemoryMapper;
#[cfg(target_os = "none")]
use crate::memory::address::VirtualAddress;
#[cfg(target_os = "none")]
use crate::{println, trace};

pub mod address;
pub mod allocator;
pub mod frame;

#[cfg(target_os = "none")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
#[cfg(target_os = "none")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
#[cfg(target_os = "none")]
pub static MEMORY_MAPPER: Once<MemoryMapper> = Once::new();

#[cfg(target_os = "none")]
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

#[cfg(target_os = "none")]
fn calculate_available_memory(entries: &[&Entry]) -> usize {
    entries
        .iter()
        .filter(|entry| entry.entry_type == EntryType::USABLE)
        .map(|entry| entry.length as usize)
        .sum()
}

#[cfg(all(test, target_os = "none"))]
mod tests {
    use crate::memory::allocator::FRAME_ALLOCATOR;
    use crate::memory::MEMORY_MAPPER;

    #[test(target = "qemu")]
    fn test_frame_allocator_initialized() {
        assert!(FRAME_ALLOCATOR.is_completed());
    }

    #[test(target = "qemu")]
    fn test_memory_mapper_initialized() {
        assert!(MEMORY_MAPPER.is_completed());
    }
}
