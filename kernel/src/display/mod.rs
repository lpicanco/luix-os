use core::fmt;

use limine::request::FramebufferRequest;
use spin::Mutex;

use crate::display::font::{FONT_HEIGHT, FONT_WIDTH};
use crate::{println, trace};

mod font;

static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

pub static DISPLAY: Mutex<Display> = Mutex::new(Display {
    buffer: &mut [],
    width: 0,
    height: 0,
    pitch: 0,
    current_row: FONT_HEIGHT,
    current_column: 0,
});

pub fn init() {
    let fb = FRAMEBUFFER_REQUEST
        .get_response()
        .expect("Failed to get framebuffer response")
        .framebuffers()
        .next()
        .expect("No framebuffers found");

    let width = fb.width();
    let height = fb.height();

    {
        let mut display = DISPLAY.lock();
        display.width = fb.width();
        display.height = fb.height();
        display.pitch = fb.pitch() / 4;

        let byte_len = display.pitch as usize * display.height as usize * (fb.bpp() as usize / 8);
        display.buffer =
            unsafe { core::slice::from_raw_parts_mut::<u32>(fb.addr().cast::<u32>(), byte_len) };

        trace!(
            "Display info: height: {}, width: {}, buffer len: {}, pitch: {}",
            display.height,
            display.width,
            display.buffer.len(),
            display.pitch
        );
    }
    println!("Display initialized. Resolution: {}x{}", width, height);
}

pub(crate) struct Display {
    buffer: &'static mut [u32],
    width: u64,
    height: u64,
    /// Distance between rows, in bytes. How many bytes we should skip to go one pixel down
    pitch: u64,
    current_row: u64,
    current_column: u64,
}

impl Display {
    fn draw_char(&mut self, x: u64, y: u64, char_bitmap: [u8; 8]) {
        for (row, &bits) in char_bitmap.iter().enumerate() {
            for bit in 0..8 {
                if (bits & (1 << bit)) != 0 {
                    let index = (y + row as u64) * self.pitch + x + bit as u64;
                    self.buffer[index as usize] = 0xFFFFFFFF;
                }
            }
        }
    }

    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.new_line(),
            '\t' => {
                self.current_column += FONT_WIDTH as u64 * 4;
            }
            _ => {
                let char_bitmap = font::DEFAULT_FONT_MAP[c as usize];
                self.draw_char(self.current_column, self.current_row, char_bitmap);
                self.current_column += FONT_WIDTH as u64;
            }
        }
        if self.current_column >= self.width {
            self.new_line();
        }
    }

    fn new_line(&mut self) {
        self.current_row += FONT_HEIGHT;
        self.current_column = 0;
    }

    fn write_string(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }
}

impl fmt::Write for Display {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
