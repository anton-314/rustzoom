//! Passive hotkey detection via Raw Input.
//!
//! `RIDEV_INPUTSINK` delivers mouse and keyboard input to our window even while the game has focus.
//! Raw Input only observes: nothing is blocked, delayed or injected (see ADR 0001).

use anyhow::{Context, Result};
use rustzoom_core::config::{Hotkey, MouseButton};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT,
    RIDEV_INPUTSINK, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE, RegisterRawInputDevices,
};

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

/// `RI_KEY_BREAK`: the key went up.
const KEY_BREAK: u16 = 0x01;

/// `RI_MOUSE_*_DOWN` / `RI_MOUSE_*_UP` flag pairs.
const MOUSE_BUTTON_FLAGS: [(MouseButton, u16, u16); 5] = [
    (MouseButton::Left, 0x0001, 0x0002),
    (MouseButton::Right, 0x0004, 0x0008),
    (MouseButton::Middle, 0x0010, 0x0020),
    (MouseButton::X1, 0x0040, 0x0080),
    (MouseButton::X2, 0x0100, 0x0200),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Pressed(Hotkey),
    Released(Hotkey),
}

/// Starts delivering `WM_INPUT` for all mice and keyboards to `hwnd`, also in the background.
pub fn register(hwnd: HWND) -> Result<()> {
    let device = |usage| RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: usage,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    let devices = [
        device(HID_USAGE_GENERIC_MOUSE),
        device(HID_USAGE_GENERIC_KEYBOARD),
    ];
    // SAFETY: `devices` is a valid array and the size matches the struct.
    unsafe { RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32) }
        .context("RegisterRawInputDevices")
}

/// Decodes the `WM_INPUT` message with this `lparam` into button/key transitions.
/// A single mouse event can carry several transitions.
pub fn read(lparam: LPARAM) -> Vec<InputEvent> {
    let mut raw = RAWINPUT::default();
    let mut size = size_of::<RAWINPUT>() as u32;
    // SAFETY: `raw` is a buffer of `size` bytes. Only mice and keyboards are registered, and their
    // events always fit into RAWINPUT.
    let copied = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam.0 as _),
            RID_INPUT,
            Some(std::ptr::from_mut(&mut raw).cast()),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if copied == u32::MAX || copied == 0 {
        return Vec::new();
    }

    match raw.header.dwType {
        t if t == RIM_TYPEMOUSE.0 => {
            // SAFETY: dwType says the union holds mouse data.
            let flags = unsafe { raw.data.mouse.Anonymous.Anonymous.usButtonFlags };
            MOUSE_BUTTON_FLAGS
                .iter()
                .flat_map(|&(button, down, up)| {
                    let hotkey = Hotkey::Mouse(button);
                    [
                        (flags & down != 0).then_some(InputEvent::Pressed(hotkey)),
                        (flags & up != 0).then_some(InputEvent::Released(hotkey)),
                    ]
                })
                .flatten()
                .collect()
        }
        t if t == RIM_TYPEKEYBOARD.0 => {
            // SAFETY: dwType says the union holds keyboard data.
            let keyboard = unsafe { raw.data.keyboard };
            let hotkey = Hotkey::Key(keyboard.VKey);
            if keyboard.Flags & KEY_BREAK != 0 {
                vec![InputEvent::Released(hotkey)]
            } else {
                vec![InputEvent::Pressed(hotkey)]
            }
        }
        _ => Vec::new(),
    }
}
