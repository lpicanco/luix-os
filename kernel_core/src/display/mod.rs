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

        assert_eq!(fb.bpp(), 32, "unsupported framebuffer format");
        assert_eq!(fb.pitch() % 4, 0, "unaligned framebuffer pitch");
        assert_eq!(fb.addr().align_offset(core::mem::align_of::<u32>()), 0);
        let byte_len = usize::try_from(fb.pitch())
            .unwrap()
            .checked_mul(usize::try_from(fb.height()).unwrap())
            .unwrap()
            / 4;
        assert!(byte_len <= isize::MAX as usize / 4);
        display.buffer =
            // SAFETY: boot grants exclusive framebuffer ownership. The checked pixel
            // format, alignment and pitch times height bound this u32 slice.
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
                    if let Some(pixel) = self.buffer.get_mut(index as usize) {
                        *pixel = 0xFFFFFFFF;
                    }
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
                let char_bitmap = *font::DEFAULT_FONT_MAP
                    .get(c as usize)
                    .unwrap_or(&font::DEFAULT_FONT_MAP[b'?' as usize]);
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
        if self.current_row + FONT_HEIGHT >= self.height {
            self.buffer.fill(0);
            self.current_row = 0;
        }
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
