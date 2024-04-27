use core::fmt;
use core::ops::Index;

use crate::arch::x86_64::registers;
use crate::memory::address::{PhysicalAddress, VirtualAddress};
use crate::memory::frame::PhysicalFrame;

pub unsafe fn active_level_4_table(physical_memory_offset: u64) -> &'static mut PageTable {
    let cr3 = registers::read_cr3();

    let page_table_virt = VirtualAddress::new(physical_memory_offset) + cr3.as_u64();
    let page_table_ptr: *mut PageTable = page_table_virt.as_mut_ptr();
    &mut *page_table_ptr
}

#[repr(C)]
pub struct Page {
    start_address: VirtualAddress,
}

impl Page {
    const PAGE_SIZE: u64 = 4096;
    pub fn containing_address(addr: VirtualAddress) -> Self {
        Self {
            start_address: addr.align_down(Self::PAGE_SIZE),
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

    fn is_unused(&self) -> bool {
        self.entry == 0
    }

    fn present(&self) -> bool {
        (self.entry >> 0) & 1 == 1
    }

    fn writable(&self) -> bool {
        (self.entry >> 1) & 1 == 1
    }

    fn user_accessible(&self) -> bool {
        (self.entry >> 2) & 1 == 1
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

    pub(crate) fn frame_size(&self, size: u64) -> Option<PhysicalFrame> {
        if self.present() {
            Some(PhysicalFrame::containing_address_size(
                self.physical_address().unwrap(),
                size,
            ))
        } else {
            None
        }
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
