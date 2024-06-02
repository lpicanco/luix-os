use core::fmt;
use core::ops::Add;

/// A physical memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub(crate) fn align_down(&self, align: u64) -> PhysicalAddress {
        PhysicalAddress(self.0 & !(align - 1))
    }

    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub(crate) fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

/// A virtual memory address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    const ENTRY_COUNT: usize = 512;
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub fn zeroed() -> Self {
        Self::new(0)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn align_down(&self, align: u64) -> VirtualAddress {
        VirtualAddress(self.0 & !(align - 1))
    }

    pub(crate) fn page_offset(&self) -> u64 {
        (self.0 % (1 << 12)) as u64
    }

    pub(crate) fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub(crate) fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self(ptr as *const () as u64)
    }

    const fn index(&self, index: u16) -> u16 {
        index % Self::ENTRY_COUNT as u16
    }

    #[inline]
    pub const fn p1_index(&self) -> u16 {
        self.index((self.0 >> 12) as u16)
    }

    /// Returns the 9-bit level 2 page table index.
    #[inline]
    pub const fn p2_index(&self) -> u16 {
        self.index((self.0 >> 12 >> 9) as u16)
    }

    /// Returns the 9-bit level 3 page table index.
    #[inline]
    pub const fn p3_index(&self) -> u16 {
        self.index((self.0 >> 12 >> 9 >> 9) as u16)
    }

    /// Returns the 9-bit level 4 page table index.
    #[inline]
    pub const fn p4_index(&self) -> u16 {
        self.index((self.0 >> 12 >> 9 >> 9 >> 9) as u16)
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self::new(self.0 + rhs)
    }
}

impl fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test_case]
    fn test_virtual_address() {
        let addr = VirtualAddress::new(0xDEAD_BEEF);
        assert_eq!(addr.as_u64(), 0xDEAD_BEEF);
        assert_eq!(addr.page_offset(), 0xEEF);
        assert_eq!(addr.p1_index(), 0xDB);
        assert_eq!(addr.p2_index(), 0xF5);
        assert_eq!(addr.p3_index(), 0x3);
        assert_eq!(addr.p4_index(), 0x0);
        assert_eq!(addr.align_down(0x1000).as_u64(), 0xDEAD_B000);
        assert_eq!(addr + 0x1, VirtualAddress::new(0xDEAD_BEF0));
        assert_eq!(addr + 0x32, VirtualAddress::new(0xDEAD_BF21));
    }

    #[test_case]
    fn test_physical_address() {
        let addr = PhysicalAddress::new(0xDEAD_BEEF);
        assert_eq!(addr.as_u64(), 0xDEAD_BEEF);
        assert_eq!(addr.align_down(0x1000).as_u64(), 0xDEAD_B000);
        assert_eq!(addr + 0x1, PhysicalAddress::new(0xDEAD_BEF0));
        assert_eq!(addr + 0x32, PhysicalAddress::new(0xDEAD_BF21));
    }
}
