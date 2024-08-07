use crate::drivers::keyboard::key::KeyCode;

pub enum ScanCodeSet1 {
    None,
    Escape,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Num0,
    Minus,
    Equal,
    Backspace,
    Tab,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    LeftBracket,
    RightBracket,
    Enter,
    LeftCtrl,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Semicolon,
    Quote,
    Backtick,
    LeftShift,
    Backslash,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RightShift,
    KeypadAsterisk,
    LeftAlt,
    Space,
    CapsLock,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    NumLock,
    ScrollLock,
    Keypad7,
    Keypad8,
    Keypad9,
    KeypadMinus,
    Keypad4,
    Keypad5,
    Keypad6,
    KeypadPlus,
    Keypad1,
    Keypad2,
    Keypad3,
    Keypad0,
    KeypadPeriod,
    None1,
    None2,
    F11,
    F12,
}

impl ScanCodeSet1 {
    pub fn to_keycode(&self) -> KeyCode {
        match self {
            ScanCodeSet1::Escape => KeyCode::Escape,
            ScanCodeSet1::Num1 => KeyCode::Num1,
            ScanCodeSet1::Num2 => KeyCode::Num2,
            ScanCodeSet1::Num3 => KeyCode::Num3,
            ScanCodeSet1::Num4 => KeyCode::Num4,
            ScanCodeSet1::Num5 => KeyCode::Num5,
            ScanCodeSet1::Num6 => KeyCode::Num6,
            ScanCodeSet1::Num7 => KeyCode::Num7,
            ScanCodeSet1::Num8 => KeyCode::Num8,
            ScanCodeSet1::Num9 => KeyCode::Num9,
            ScanCodeSet1::Num0 => KeyCode::Num0,
            ScanCodeSet1::Minus => KeyCode::Minus,
            ScanCodeSet1::Equal => KeyCode::Equal,
            ScanCodeSet1::Backspace => KeyCode::Backspace,
            ScanCodeSet1::Tab => KeyCode::Tab,
            ScanCodeSet1::Q => KeyCode::Q,
            ScanCodeSet1::W => KeyCode::W,
            ScanCodeSet1::E => KeyCode::E,
            ScanCodeSet1::R => KeyCode::R,
            ScanCodeSet1::T => KeyCode::T,
            ScanCodeSet1::Y => KeyCode::Y,
            ScanCodeSet1::U => KeyCode::U,
            ScanCodeSet1::I => KeyCode::I,
            ScanCodeSet1::O => KeyCode::O,
            ScanCodeSet1::P => KeyCode::P,
            ScanCodeSet1::LeftBracket => KeyCode::LeftBracket,
            ScanCodeSet1::RightBracket => KeyCode::RightBracket,
            ScanCodeSet1::Enter => KeyCode::Enter,
            ScanCodeSet1::LeftCtrl => KeyCode::LeftCtrl,
            ScanCodeSet1::A => KeyCode::A,
            ScanCodeSet1::S => KeyCode::S,
            ScanCodeSet1::D => KeyCode::D,
            ScanCodeSet1::F => KeyCode::F,
            ScanCodeSet1::G => KeyCode::G,
            ScanCodeSet1::H => KeyCode::H,
            ScanCodeSet1::J => KeyCode::J,
            ScanCodeSet1::K => KeyCode::K,
            ScanCodeSet1::L => KeyCode::L,
            ScanCodeSet1::Semicolon => KeyCode::Semicolon,
            ScanCodeSet1::Quote => KeyCode::Quote,
            ScanCodeSet1::Backtick => KeyCode::Backtick,
            ScanCodeSet1::LeftShift => KeyCode::LeftShift,
            ScanCodeSet1::Backslash => KeyCode::Backslash,
            ScanCodeSet1::Z => KeyCode::Z,
            ScanCodeSet1::X => KeyCode::X,
            ScanCodeSet1::C => KeyCode::C,
            ScanCodeSet1::V => KeyCode::V,
            ScanCodeSet1::B => KeyCode::B,
            ScanCodeSet1::N => KeyCode::N,
            ScanCodeSet1::M => KeyCode::M,
            ScanCodeSet1::Comma => KeyCode::Comma,
            ScanCodeSet1::Period => KeyCode::Period,
            ScanCodeSet1::Slash => KeyCode::Slash,
            ScanCodeSet1::RightShift => KeyCode::RightShift,
            ScanCodeSet1::KeypadAsterisk => KeyCode::KeypadAsterisk,
            ScanCodeSet1::LeftAlt => KeyCode::LeftAlt,
            ScanCodeSet1::Space => KeyCode::Space,
            ScanCodeSet1::CapsLock => KeyCode::CapsLock,
            ScanCodeSet1::F1 => KeyCode::F1,
            ScanCodeSet1::F2 => KeyCode::F2,
            ScanCodeSet1::F3 => KeyCode::F3,
            ScanCodeSet1::F4 => KeyCode::F4,
            ScanCodeSet1::F5 => KeyCode::F5,
            ScanCodeSet1::F6 => KeyCode::F6,
            ScanCodeSet1::F7 => KeyCode::F7,
            ScanCodeSet1::F8 => KeyCode::F8,
            ScanCodeSet1::F9 => KeyCode::F9,
            ScanCodeSet1::F10 => KeyCode::F10,
            ScanCodeSet1::NumLock => KeyCode::NumLock,
            ScanCodeSet1::ScrollLock => KeyCode::ScrollLock,
            ScanCodeSet1::Keypad7 => KeyCode::Keypad7,
            ScanCodeSet1::Keypad8 => KeyCode::Keypad8,
            ScanCodeSet1::Keypad9 => KeyCode::Keypad9,
            ScanCodeSet1::KeypadMinus => KeyCode::KeypadMinus,
            ScanCodeSet1::Keypad4 => KeyCode::Keypad4,
            ScanCodeSet1::Keypad5 => KeyCode::Keypad5,
            ScanCodeSet1::Keypad6 => KeyCode::Keypad6,
            ScanCodeSet1::KeypadPlus => KeyCode::KeypadPlus,
            ScanCodeSet1::Keypad1 => KeyCode::Keypad1,
            ScanCodeSet1::Keypad2 => KeyCode::Keypad2,
            ScanCodeSet1::Keypad3 => KeyCode::Keypad3,
            ScanCodeSet1::Keypad0 => KeyCode::Keypad0,
            ScanCodeSet1::KeypadPeriod => KeyCode::KeypadPeriod,
            ScanCodeSet1::F11 => KeyCode::F11,
            ScanCodeSet1::F12 => KeyCode::F12,
            _ => KeyCode::Unknown,
        }
    }
}
