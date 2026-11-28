use spin::Mutex;

use crate::drivers::keyboard::key::KeyEvent;
use crate::drivers::keyboard::set1::ScanCodeSet1;
use crate::utils::ring_buffer::RingBuffer;

mod key;
mod set1;

pub static KEY_BUFFER: Mutex<RingBuffer<KeyEvent, 30>> =
    Mutex::new(RingBuffer::<KeyEvent, 30>::new());

pub fn handle_key(key: u8) {
    if let Some(scan_code) = ScanCodeSet1::from_byte(key) {
        let event = KeyEvent {
            key_code: scan_code.to_keycode(),
            key,
        };
        KEY_BUFFER.lock().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_key, key::KeyCode, KeyEvent, RingBuffer, KEY_BUFFER};

    #[test]
    fn queues_key_presses_and_ignores_releases_and_unsupported_codes() {
        let presses = [
            (0x01, KeyCode::Escape, None),
            (0x02, KeyCode::Num1, Some('1')),
            (0x1e, KeyCode::A, Some('a')),
            (0x1c, KeyCode::Enter, Some('\n')),
            (0x39, KeyCode::Space, Some(' ')),
            (0x4f, KeyCode::Keypad1, Some('1')),
            (0x53, KeyCode::KeypadPeriod, Some('.')),
            (0x57, KeyCode::F11, None),
            (0x58, KeyCode::F12, None),
        ];
        *KEY_BUFFER.lock() = RingBuffer::new();

        for (scan_code, _, _) in presses {
            handle_key(scan_code);
            handle_key(scan_code | 0x80);
        }
        for scan_code in [0x00, 0x54, 0x55, 0x56, 0x59, 0x7f, 0x80, 0xe0, 0xe1, 0xff] {
            handle_key(scan_code);
        }

        let mut buffer = KEY_BUFFER.lock();
        for (scan_code, key_code, character) in presses {
            let event = buffer.pop().expect("key press must be queued");
            assert_eq!(
                event,
                KeyEvent {
                    key: scan_code,
                    key_code
                }
            );
            assert_eq!(event.decode(), character);
        }
        assert_eq!(buffer.pop(), None);
    }
}
