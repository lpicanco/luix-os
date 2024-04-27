use crate::arch::memory::paging::{PageTable};
use crate::arch::registers::read_cr3;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;

pub struct MemoryMapper {
    physical_memory_offset: VirtualAddress
}

impl MemoryMapper {
    pub fn new(physical_memory_offset: VirtualAddress) -> Self {
        MemoryMapper {
            physical_memory_offset
        }
    }

    pub fn translate_addr(&self, addr: VirtualAddress) -> Option<PhysicalAddress> {
        let mut frame = PhysicalFrame::containing_address(read_cr3());

        let table_indexes = [
            addr.p4_index(),
            addr.p3_index(),
            addr.p2_index(),
            addr.p1_index(),
        ];

        for (i, &index) in table_indexes.iter().enumerate() {
            let page_table_virt = self.physical_memory_offset + frame.start_address.as_u64();

            let page_table_ptr: *const PageTable = page_table_virt.as_ptr();
            let page_table = unsafe { &*page_table_ptr };
            let page_table_entry = &page_table[index as usize];

            if page_table_entry.huge_page() {
                return match i {
                    // P3: 1GiB page
                    1 => {
                        let offset = addr.as_u64() & 0o_777_777_7777;
                        Some(page_table_entry.frame().unwrap().start_address + offset)
                    }
                    // P2: 2MiB page
                    2 => {
                        let offset = addr.as_u64() & 0o_777_7777;
                        Some(page_table_entry.frame().unwrap().start_address + offset)
                    }
                    _ => {
                        None
                    }
                }
            } else {
                frame = match page_table_entry.frame() {
                    Some(frame) => frame,
                    None => return None,
                };
            }
        }

        // Calculate the physical address by adding the page offset
        Some(frame.start_address + addr.page_offset())
    }
}