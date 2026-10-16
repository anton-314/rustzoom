//! User settings. For now only the built-in defaults; loading from a file comes later.

use crate::zoom::ZoomMode;

pub const DEFAULT_FACTOR: f32 = 3.0;

/// Mouse buttons that can be bound as hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    /// "Mouse 4", usually the rear side button.
    X1,
    /// "Mouse 5", usually the front side button.
    X2,
}

/// A bindable input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    /// Windows virtual-key code (`VK_*`).
    Key(u16),
    Mouse(MouseButton),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub hotkey: Hotkey,
    pub mode: ZoomMode,
    /// Selectable magnification factors, the first one is active at start.
    pub levels: Vec<f32>,
    /// Scope diameter relative to the monitor height (0.0–1.0).
    pub scope_diameter: f32,
    /// Width of the scope border in pixels.
    pub border_width: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: Hotkey::Mouse(MouseButton::X1),
            mode: ZoomMode::Hold,
            levels: vec![DEFAULT_FACTOR],
            scope_diameter: 0.4,
            border_width: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_mvp() {
        let config = Config::default();
        assert_eq!(config.hotkey, Hotkey::Mouse(MouseButton::X1));
        assert_eq!(config.mode, ZoomMode::Hold);
        assert_eq!(config.levels, vec![3.0]);
        assert_eq!(config.scope_diameter, 0.4);
        assert_eq!(config.border_width, 2.0);
    }
}
