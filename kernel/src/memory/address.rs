use core::fmt;
use core::ops::Add;

/// A physical memory address.
#[derive(Debug)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub(crate) fn align_down(&self, align: u64) -> PhysicalAddress {
        PhysicalAddress(self.0 & !(align - 1))
    }

    pub fn new(addr: u64) -> Self {
        Self(addr)
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    const ENTRY_COUNT: usize = 512;
    pub fn new(addr: u64) -> Self {
        Self(addr)
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
