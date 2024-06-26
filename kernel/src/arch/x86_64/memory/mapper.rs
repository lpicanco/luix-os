use crate::allocate_frame;
use crate::arch::memory::paging::{Page, PageTable, PageTableEntry};
use crate::arch::registers::read_cr3;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;

pub struct MemoryMapper {
    pub physical_memory_offset: VirtualAddress,
}

impl MemoryMapper {
    pub fn new(physical_memory_offset: VirtualAddress) -> Self {
        MemoryMapper {
            physical_memory_offset,
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
                    _ => None,
                };
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

    pub(crate) fn map_page(
        &self,
        page: Page,
        frame: PhysicalFrame,
        user_accessible: bool,
        writable: bool,
    ) {
        let mut current_frame = PhysicalFrame::containing_address(read_cr3());
        let table_indexes = [
            page.p4_index(),
            page.p3_index(),
            page.p2_index(),
            page.p1_index(),
        ];

        for (i, &index) in table_indexes.iter().enumerate() {
            let page_table_virt =
                self.physical_memory_offset + current_frame.start_address.as_u64();

            let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();
            let page_table = unsafe { &mut *page_table_ptr };
            let page_table_entry = &mut page_table[index as usize];

            match page_table_entry.frame() {
                Some(entry_frame) => {
                    // At L1, the page should not be mapped
                    if i == 3 && !page_table_entry.is_unused() {
                        // TODO: Handle unused frames
                        panic!(
                            "Page already mapped to frame: {}. Entry: {}",
                            entry_frame.start_address, page_table_entry
                        );
                    }
                    current_frame = entry_frame;
                }
                None => {
                    // At L1, associate the page with the frame
                    if i == 3 {
                        current_frame = frame.clone();
                    } else {
                        current_frame = allocate_frame!();
                    }
                }
            };
            self.map_page_entry(
                page_table_entry,
                current_frame.clone(),
                user_accessible,
                writable,
            )
        }
    }

    pub fn unmap_page(&self, page: Page) {
        let mut frame = PhysicalFrame::containing_address(read_cr3());

        let table_indexes = [
            page.p4_index(),
            page.p3_index(),
            page.p2_index(),
            page.p1_index(),
        ];

        let mut page_table_entry: Option<&mut PageTableEntry> = None;

        for &index in &table_indexes {
            let page_table_virt = self.physical_memory_offset + frame.start_address.as_u64();

            let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();
            let page_table = unsafe { &mut *page_table_ptr };
            page_table_entry = Some(&mut page_table[index as usize]);

            if page_table_entry.as_ref().is_some_and(|pte| pte.huge_page()) {
                break;
            }

            frame = match page_table_entry.as_ref().unwrap().frame() {
                Some(frame) => frame,
                None => return,
            };
        }

        if let Some(page_table_entry) = page_table_entry {
            page_table_entry.set_unused()
        }
    }

    fn map_page_entry(
        &self,
        page_table_entry: &mut PageTableEntry,
        frame: PhysicalFrame,
        user_accessible: bool,
        writable: bool,
    ) {
        page_table_entry.set_present();
        if user_accessible {
            page_table_entry.set_user_accessible();
        }
        if writable {
            page_table_entry.set_writable();
        }
        page_table_entry.set_frame(frame);
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::MEMORY_MAPPER;

    use super::*;

    #[test_case]
    fn test_translate_addr() {
        let mapper = unsafe { MEMORY_MAPPER.get_unchecked() };
        let phys = mapper.translate_addr(mapper.physical_memory_offset);
        assert_eq!(phys, Some(PhysicalAddress::new(0)));

        let phys = mapper.translate_addr(mapper.physical_memory_offset + 0x42);
        assert_eq!(phys, Some(PhysicalAddress::new(0x42)));

        let phys = mapper.translate_addr(VirtualAddress::new(0xFEED_CAFE_000));
        assert_eq!(phys, Some(PhysicalAddress::new(0x1000)));

        let phys = mapper.translate_addr(VirtualAddress::new(0xFEEDCB0A000));
        assert_eq!(phys, Some(PhysicalAddress::new(0x10000)));

        let phys = mapper.translate_addr(VirtualAddress::new(0xFEED_DEAD_0000));
        assert_eq!(phys, None);
    }

    #[test_case]
    fn test_map_page() {
        let mapper = unsafe { MEMORY_MAPPER.get_unchecked() };

        // check that the page is not mapped
        let virt = VirtualAddress::new(0xFEED_DEAD_1000);
        let phys = mapper.translate_addr(virt);
        assert_eq!(phys, None);

        // map the page
        let page = Page::containing_address(virt);
        let frame = allocate_frame!();
        mapper.map_page(page, frame.clone(), false, false);

        // check that the page is mapped
        let phys = mapper.translate_addr(virt);
        assert_eq!(phys, Some(frame.start_address));
    }

    #[test_case]
    fn test_unmap_page() {
        let mapper = unsafe { MEMORY_MAPPER.get_unchecked() };

        // map the page
        let virt = VirtualAddress::new(0xFEED_DEAD_2000);
        let page = Page::containing_address(virt);
        let frame = allocate_frame!();
        mapper.map_page(page, frame.clone(), false, false);

        // check that the page is mapped
        let phys = mapper.translate_addr(virt);
        assert_eq!(phys, Some(frame.start_address));

        // unmap the page
        mapper.unmap_page(page);

        // check that the page is not mapped
        let phys = mapper.translate_addr(virt);
        assert_eq!(phys, None);

        // check that the page can be remapped
        let frame = allocate_frame!();
        mapper.map_page(page, frame.clone(), false, false);
        let phys = mapper.translate_addr(virt);
        assert_eq!(phys, Some(frame.start_address));
    }
}
