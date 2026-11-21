// Host test builds compile the hardware-independent x86_64 types on any host arch.
#[cfg(any(target_arch = "x86_64", not(target_os = "none")))]
pub(crate) use self::x86_64::*;

#[cfg(any(target_arch = "x86_64", not(target_os = "none")))]
pub(crate) mod x86_64;
