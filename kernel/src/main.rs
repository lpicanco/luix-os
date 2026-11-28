#![no_std]
#![no_main]
#![forbid(unsafe_code)]

#[kernel_core::entry]
fn start() -> ! {
    kernel::run()
}
