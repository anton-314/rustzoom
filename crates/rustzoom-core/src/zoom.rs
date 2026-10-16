//! Hotkey → zoom state machine.

use serde::{Deserialize, Serialize};

/// How the hotkey activates the zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoomMode {
    /// Zoomed while the hotkey is held down.
    Hold,
    /// Each press switches the zoom on or off.
    Toggle,
}

/// What the overlay should show right now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomState {
    pub active: bool,
    pub factor: f32,
}

/// Turns hotkey presses/releases into a [`ZoomState`].
#[derive(Debug, Clone)]
pub struct ZoomController {
    mode: ZoomMode,
    levels: Vec<f32>,
    level: usize,
    active: bool,
    pressed: bool,
}

impl ZoomController {
    /// `levels` are the selectable magnification factors; the first one is selected initially.
    /// An empty list falls back to a single level of [`crate::config::DEFAULT_FACTOR`].
    pub fn new(mode: ZoomMode, levels: Vec<f32>) -> Self {
        let levels = if levels.is_empty() {
            vec![crate::config::DEFAULT_FACTOR]
        } else {
            levels
        };
        Self {
            mode,
            levels,
            level: 0,
            active: false,
            pressed: false,
        }
    }

    pub fn state(&self) -> ZoomState {
        ZoomState {
            active: self.active,
            factor: self.levels[self.level],
        }
    }

    /// Hotkey went down. Repeated presses without a release (keyboard auto-repeat) are ignored.
    pub fn on_press(&mut self) -> ZoomState {
        if !self.pressed {
            self.pressed = true;
            self.active = match self.mode {
                ZoomMode::Hold => true,
                ZoomMode::Toggle => !self.active,
            };
        }
        self.state()
    }

    /// Hotkey went up.
    pub fn on_release(&mut self) -> ZoomState {
        self.pressed = false;
        if self.mode == ZoomMode::Hold {
            self.active = false;
        }
        self.state()
    }

    /// Selects the next magnification level, wrapping around. Works whether or not zoom is active;
    /// the selection is kept when zoom is switched off.
    pub fn cycle_level(&mut self) -> ZoomState {
        self.level = (self.level + 1) % self.levels.len();
        self.state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_is_active_only_while_pressed() {
        let mut zoom = ZoomController::new(ZoomMode::Hold, vec![3.0]);
        assert!(!zoom.state().active);
        assert!(zoom.on_press().active);
        assert!(zoom.on_press().active, "auto-repeat keeps it active");
        assert!(!zoom.on_release().active);
    }

    #[test]
    fn toggle_flips_on_press_and_ignores_release() {
        let mut zoom = ZoomController::new(ZoomMode::Toggle, vec![3.0]);
        assert!(zoom.on_press().active);
        assert!(zoom.on_release().active);
        assert!(!zoom.on_press().active);
        assert!(!zoom.on_release().active);
    }

    #[test]
    fn toggle_ignores_auto_repeat() {
        let mut zoom = ZoomController::new(ZoomMode::Toggle, vec![3.0]);
        zoom.on_press();
        assert!(zoom.on_press().active);
        assert!(zoom.on_press().active);
    }

    #[test]
    fn levels_cycle_and_wrap() {
        let mut zoom = ZoomController::new(ZoomMode::Hold, vec![2.0, 4.0, 8.0]);
        assert_eq!(zoom.state().factor, 2.0);
        assert_eq!(zoom.cycle_level().factor, 4.0);
        assert_eq!(zoom.cycle_level().factor, 8.0);
        assert_eq!(zoom.cycle_level().factor, 2.0);
    }

    #[test]
    fn level_survives_deactivation() {
        let mut zoom = ZoomController::new(ZoomMode::Hold, vec![2.0, 4.0]);
        zoom.on_press();
        zoom.cycle_level();
        let state = zoom.on_release();
        assert!(!state.active);
        assert_eq!(state.factor, 4.0);
        assert_eq!(zoom.on_press().factor, 4.0);
    }

    #[test]
    fn empty_levels_fall_back_to_default() {
        let zoom = ZoomController::new(ZoomMode::Hold, Vec::new());
        assert_eq!(zoom.state().factor, crate::config::DEFAULT_FACTOR);
    }
}
