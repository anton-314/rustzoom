//! Display names for hotkeys.

use crate::config::{Hotkey, MouseButton};

/// Human-readable name, e.g. "Mouse 4", "Alt", "F", "F5". Key names are the same in every UI
/// language (as in games). Keys without a fixed name show their code.
pub fn hotkey_name(hotkey: Hotkey) -> String {
    match hotkey {
        Hotkey::Mouse(button) => match button {
            MouseButton::Left => "Mouse 1",
            MouseButton::Right => "Mouse 2",
            MouseButton::Middle => "Mouse 3",
            MouseButton::X1 => "Mouse 4",
            MouseButton::X2 => "Mouse 5",
        }
        .to_owned(),
        Hotkey::Key(vk) => key_name(vk),
    }
}

fn key_name(vk: u16) -> String {
    let fixed = match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        0x6A => "Num *",
        0x6B => "Num +",
        0x6D => "Num -",
        0x6E => "Num .",
        0x6F => "Num /",
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",
        0xA0 => "Left Shift",
        0xA1 => "Right Shift",
        0xA2 => "Left Ctrl",
        0xA3 => "Right Ctrl",
        0xA4 => "Left Alt",
        0xA5 => "Right Alt",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_owned();
    }
    match vk {
        // 0-9 and A-Z have the ASCII code as VK code.
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => format!("Key 0x{vk:02X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(vk: u16) -> String {
        hotkey_name(Hotkey::Key(vk))
    }

    #[test]
    fn mouse_buttons() {
        assert_eq!(hotkey_name(Hotkey::Mouse(MouseButton::X1)), "Mouse 4");
        assert_eq!(hotkey_name(Hotkey::Mouse(MouseButton::X2)), "Mouse 5");
        assert_eq!(hotkey_name(Hotkey::Mouse(MouseButton::Right)), "Mouse 2");
    }

    #[test]
    fn letters_digits_function_keys() {
        assert_eq!(key(0x41), "A");
        assert_eq!(key(0x46), "F");
        assert_eq!(key(0x5A), "Z");
        assert_eq!(key(0x30), "0");
        assert_eq!(key(0x39), "9");
        assert_eq!(key(0x70), "F1");
        assert_eq!(key(0x87), "F24");
        assert_eq!(key(0x63), "Num 3");
    }

    #[test]
    fn modifiers_and_specials() {
        assert_eq!(key(0x12), "Alt");
        assert_eq!(key(0x10), "Shift");
        assert_eq!(key(0x11), "Ctrl");
        assert_eq!(key(0x20), "Space");
    }

    #[test]
    fn unknown_code() {
        assert_eq!(key(0xC0), "Key 0xC0");
        assert_eq!(key(0x07), "Key 0x07");
    }
}
