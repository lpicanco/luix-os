use core::fmt;
use core::ops::{Index, IndexMut};

use crate::arch::x86_64::registers;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;

pub unsafe fn active_level_4_table(physical_memory_offset: u64) -> &'static mut PageTable {
    let cr3 = registers::read_cr3();

    let page_table_virt = VirtualAddress::new(physical_memory_offset) + cr3.as_u64();
    let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();
    &mut *page_table_ptr
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Page {
    pub start_address: VirtualAddress,
}

impl Page {
    const PAGE_SIZE: u64 = 4096;
    pub fn p4_index(&self) -> u16 {
        self.start_address.p4_index()
    }

    pub fn p3_index(&self) -> u16 {
        self.start_address.p3_index()
    }

    pub fn p2_index(&self) -> u16 {
        self.start_address.p2_index()
    }

    pub fn p1_index(&self) -> u16 {
        self.start_address.p1_index()
    }

    pub fn containing_address(addr: VirtualAddress) -> Self {
        Self {
            start_address: addr.align_down(Self::PAGE_SIZE),
        }
    }
    pub(crate) fn range_inclusive(start: Page, end: Page) -> PageIter {
        PageIter { start, end }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIter {
    start: Page,
    end: Page,
}

impl Iterator for PageIter {
    type Item = Page;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let page = self.start;
            self.start = Page {
                start_address: page.start_address + Page::PAGE_SIZE,
            };
            Some(page)
        } else {
            None
        }
    }
}

pub(crate) struct PageTable {
    entries: [PageTableEntry; 512],
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}
impl PageTableEntry {
    // Mask to get the physical address from the entry
    const PHYSICAL_ADDRESS_MASK: u64 = 0x000fffff_fffff000;

    fn new() -> Self {
        Self { entry: 0 }
    }

    pub fn is_unused(&self) -> bool {
        self.entry == 0
    }

    pub fn present(&self) -> bool {
        (self.entry >> 0) & 1 == 1
    }

    pub fn set_present(&mut self) {
        self.entry |= 1 << 0;
    }

    pub fn writable(&self) -> bool {
        (self.entry >> 1) & 1 == 1
    }

    pub fn set_writable(&mut self) {
        self.entry |= 1 << 1;
    }

    fn user_accessible(&self) -> bool {
        (self.entry >> 2) & 1 == 1
    }

    pub fn set_user_accessible(&mut self) {
        self.entry |= 1 << 2;
    }

    fn write_through(&self) -> bool {
        (self.entry >> 3) & 1 == 1
    }

    fn accessed(&self) -> bool {
        (self.entry >> 5) & 1 == 1
    }

    fn dirty(&self) -> bool {
        (self.entry >> 6) & 1 == 1
    }

    pub(crate) fn huge_page(&self) -> bool {
        (self.entry >> 7) & 1 == 1
    }

    pub(crate) fn frame(&self) -> Option<PhysicalFrame> {
        if self.present() {
            Some(PhysicalFrame::containing_address(
                self.physical_address().unwrap(),
            ))
        } else {
            None
        }
    }

    pub(crate) fn set_frame(&mut self, frame: PhysicalFrame) {
        // clear the address bits
        self.entry &= !Self::PHYSICAL_ADDRESS_MASK;

        // set the address bits
        self.entry |= frame.start_address.as_u64() & Self::PHYSICAL_ADDRESS_MASK;
    }

    fn physical_address(&self) -> Option<PhysicalAddress> {
        if self.present() {
            Some(PhysicalAddress::new(
                self.entry & Self::PHYSICAL_ADDRESS_MASK,
            ))
        } else {
            None
        }
    }
}

impl fmt::Display for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entry: {:#x}", self.entry)?;
        write!(
            f,
            ", physical address: {:#x}",
            self.physical_address().unwrap().as_u64()
        )?;
        write!(f, ", present: {}", self.present())?;
        write!(f, ", writable: {}", self.writable())?;
        write!(f, ", user accessible: {}", self.user_accessible())?;
        write!(f, ", write through: {}", self.write_through())?;
        write!(f, ", accessed: {}", self.accessed())?;
        write!(f, ", dirty: {}", self.dirty())?;
        write!(f, ", huge page: {}", self.huge_page())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_page_containing_address() {
        let page = Page::containing_address(VirtualAddress::new(0x1000));
        assert_eq!(page.start_address, VirtualAddress::new(0x1000));

        let page = Page::containing_address(VirtualAddress::new(0x1001));
        assert_eq!(page.start_address, VirtualAddress::new(0x1000));
    }

    #[test_case]
    fn test_page_level() {
        let page = Page::containing_address(VirtualAddress::new(0xDEAD_BEEF));
        assert_eq!(page.p1_index(), 0xDB);
        assert_eq!(page.p2_index(), 0xF5);
        assert_eq!(page.p3_index(), 0x3);
        assert_eq!(page.p4_index(), 0x0);
    }

    #[test_case]
    fn test_page_iter() {
        let start = Page::containing_address(VirtualAddress::new(0x1000));
        let end = Page::containing_address(VirtualAddress::new(0x3005));
        let mut iter = Page::range_inclusive(start, end);
        assert_eq!(iter.next(), Some(start));
        assert_eq!(
            iter.next(),
            Some(Page::containing_address(VirtualAddress::new(0x2000)))
        );
        assert_eq!(
            iter.next(),
            Some(Page::containing_address(VirtualAddress::new(0x3000)))
        );
        assert_eq!(iter.next(), None);
    }

    #[test_case]
    fn test_page_table_entry() {
        let mut entry = PageTableEntry::new();
        assert_eq!(entry.is_unused(), true);
        assert_eq!(entry.present(), false);
        assert_eq!(entry.writable(), false);
        assert_eq!(entry.user_accessible(), false);
        assert_eq!(entry.write_through(), false);
        assert_eq!(entry.accessed(), false);
        assert_eq!(entry.dirty(), false);
        assert_eq!(entry.huge_page(), false);

        entry.set_present();
        assert_eq!(entry.present(), true);

        entry.set_writable();
        assert_eq!(entry.writable(), true);

        entry.set_user_accessible();
        assert_eq!(entry.user_accessible(), true);
    }
}
