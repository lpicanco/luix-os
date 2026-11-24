use core::arch::asm;
use core::ptr;

use crate::allocate_frame;
use crate::arch::memory::paging::{Page, PageTable, PageTableEntry, PAGE_SIZE};
use crate::arch::registers::read_cr3;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;

struct Mapping {
    physical: PhysicalAddress,
    uncached: bool,
    writable: bool,
    user_accessible: bool,
}

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
        self.mapping(addr).map(|mapping| mapping.physical)
    }

    fn mapping(&self, addr: VirtualAddress) -> Option<Mapping> {
        let mut frame = PhysicalFrame::containing_address(read_cr3());
        let mut writable = true;
        let mut user_accessible = true;
        for (index, shift) in [
            (addr.p4_index(), 39),
            (addr.p3_index(), 30),
            (addr.p2_index(), 21),
            (addr.p1_index(), 12),
        ] {
            let table = self.physical_memory_offset + frame.start_address.as_u64();
            let table = unsafe { &*table.as_ptr::<PageTable>() };
            let entry = &table[usize::from(index)];
            frame = entry.frame()?;
            writable &= entry.writable();
            user_accessible &= entry.user_accessible();
            if shift == 12 || (shift != 39 && entry.huge_page()) {
                return Some(Mapping {
                    physical: entry.leaf_address(addr, shift),
                    uncached: entry.is_uncacheable(shift),
                    writable,
                    user_accessible,
                });
            }
        }
        None
    }

    pub(crate) fn map_page(
        &self,
        page: Page,
        frame: PhysicalFrame,
        user_accessible: bool,
        writable: bool,
    ) {
        self.map_page_with_cache(page, frame, user_accessible, writable, false);
    }

    pub(crate) fn map_mmio_page(&self, page: Page, frame: PhysicalFrame) -> Option<()> {
        let physical = frame.start_address;
        // Limine can also install a low identity alias. Cacheable and uncacheable
        // mappings of the same device registers must never be active together.
        if physical.as_u64() < 1 << 47 {
            if let Some(alias) = self.mapping(VirtualAddress::new(physical.as_u64())) {
                if alias.physical == physical && (!alias.uncached || alias.user_accessible) {
                    return None;
                }
            }
        }
        match self.mapping(page.start_address) {
            Some(m) => (m.physical == physical && m.uncached && m.writable && !m.user_accessible)
                .then_some(()),
            None => {
                self.map_page_with_cache(page, frame, false, true, true);
                Some(())
            }
        }
    }

    fn map_page_with_cache(
        &self,
        page: Page,
        frame: PhysicalFrame,
        user_accessible: bool,
        writable: bool,
        uncached: bool,
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
                        let table =
                            self.physical_memory_offset + current_frame.start_address.as_u64();
                        unsafe {
                            ptr::write_bytes(table.as_mut_ptr::<u8>(), 0, PAGE_SIZE as usize)
                        };
                    }
                }
            };
            if uncached && i == 3 {
                page_table_entry.set_uncacheable();
            }
            self.map_page_entry(
                page_table_entry,
                current_frame.clone(),
                user_accessible,
                writable,
            )
        }
    }

    pub fn unmap_page(&self, page: Page) -> Option<PhysicalFrame> {
        let mut frame = PhysicalFrame::containing_address(read_cr3());

        let table_indexes = [
            page.p4_index(),
            page.p3_index(),
            page.p2_index(),
            page.p1_index(),
        ];

        for (level, &index) in table_indexes.iter().enumerate() {
            let page_table_virt = self.physical_memory_offset + frame.start_address.as_u64();

            let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();
            let page_table = unsafe { &mut *page_table_ptr };
            let entry = &mut page_table[index as usize];
            frame = entry.frame()?;
            if level == table_indexes.len() - 1 || entry.huge_page() {
                entry.set_unused();
                unsafe {
                    asm!("invlpg [{}]", in(reg) page.start_address.as_u64(), options(nostack, preserves_flags));
                }
                return Some(frame);
            }
        }
        None
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

    #[test(target = "qemu")]
    fn test_translate_addr() {
        let mapper = unsafe { MEMORY_MAPPER.get_unchecked() };
        let phys = mapper.translate_addr(mapper.physical_memory_offset);
        assert_eq!(phys, Some(PhysicalAddress::new(0)));

        let phys = mapper.translate_addr(mapper.physical_memory_offset + 0x42);
        assert_eq!(phys, Some(PhysicalAddress::new(0x42)));

        let virt = VirtualAddress::new(0xFEED_DEAD_3000);
        let frame = allocate_frame!();
        mapper.map_page(Page::containing_address(virt), frame.clone(), false, false);
        assert_eq!(mapper.translate_addr(virt), Some(frame.start_address));
        assert_eq!(
            mapper.translate_addr(virt + 0x42),
            Some(frame.start_address + 0x42)
        );

        let phys = mapper.translate_addr(VirtualAddress::new(0xFEED_DEAD_0000));
        assert_eq!(phys, None);
    }

    #[test(target = "qemu")]
    fn mmio_rejects_existing_ram_mapping() {
        let mapper = MEMORY_MAPPER.get().unwrap();
        let frame = PhysicalFrame::containing_address(read_cr3());
        let page =
            Page::containing_address(mapper.physical_memory_offset + frame.start_address.as_u64());
        assert_eq!(mapper.map_mmio_page(page, frame), None);
    }

    #[test(target = "qemu")]
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

    #[test(target = "qemu")]
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
        assert_eq!(mapper.unmap_page(page), Some(frame));

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
