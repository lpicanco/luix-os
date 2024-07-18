use core::mem;

use spin::Mutex;

use crate::drivers::keyboard::key::KeyEvent;
use crate::drivers::keyboard::set1::ScanCodeSet1;
use crate::utils::ring_buffer::RingBuffer;

mod key;
mod set1;

pub static KEY_BUFFER: Mutex<RingBuffer<KeyEvent, 30>> =
    Mutex::new(RingBuffer::<KeyEvent, 30>::new());

pub fn handle_key(key: u8) {
    unsafe {
        if key & 0x80 != 0 {
            // TODO: Handle key release
            return;
        } else {
            // For now, we only support scan code set 1
            let scan_code = mem::transmute::<u8, ScanCodeSet1>(key);
            let key_code = scan_code.to_keycode();
            let event = KeyEvent { key_code, key };
            KEY_BUFFER.lock().push(event);
        }
    }
}

mod tests {
    use alloc::collections::VecDeque;

    #[test_case]
    fn test_key_buffer() {
        let mut ring_buffer = VecDeque::with_capacity(3);

        ring_buffer.push_back(1);
        ring_buffer.push_back(2);
        ring_buffer.push_back(3);
        ring_buffer.push_back(4);
        ring_buffer.push_back(5);
        assert_eq!(ring_buffer.pop_front(), Some(1));
        assert_eq!(ring_buffer.pop_front(), Some(2));
        assert_eq!(ring_buffer.pop_front(), Some(3));
    }
}
