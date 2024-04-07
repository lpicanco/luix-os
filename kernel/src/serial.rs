use core::fmt;
use spin::Mutex;

pub(crate) static SERIAL: Mutex<Serial> = Mutex::new(Serial {});

pub(crate) trait SerialWriter {
    fn write_string(&mut self, s: &str);
}

pub(crate) struct Serial {}

impl Serial {}

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
