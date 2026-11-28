//! Console and serial formatting without raw hardware handles.
use core::fmt;

use crate::display::DISPLAY;
use crate::serial::SERIAL;

/// Write formatted output to the serial console and framebuffer.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

/// Write a formatted line to the serial console and framebuffer.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// Write a formatted diagnostic line to the serial console.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => ($crate::print::_trace(format_args!("{}\n", format_args!($($arg)*))));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    SERIAL
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
    DISPLAY
        .lock()
        .write_fmt(args)
        .expect("Printing to display failed");
}

#[doc(hidden)]
pub fn _trace(args: fmt::Arguments) {
    use core::fmt::Write;
    SERIAL
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}
