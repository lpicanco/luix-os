#![no_std]
#![cfg_attr(not(feature = "user"), forbid(unsafe_code))]

#[cfg(feature = "user")]
pub mod arch;
pub mod syscall;
