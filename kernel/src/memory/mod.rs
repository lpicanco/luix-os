use limine::memory_map::{Entry, EntryType};
use limine::request::{HhdmRequest, MemoryMapRequest};

use crate::{println, trace};

mod allocator;

static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

pub fn init() {
    let pm_offset = HHDM_REQUEST
        .get_response()
        .expect("Failed to get PM offset")
        .offset();
    trace!("Physical memory offset: {:#X}", pm_offset);

    let memory_map = MEMORY_MAP_REQUEST
        .get_response()
        .expect("Failed to get memory map");

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
