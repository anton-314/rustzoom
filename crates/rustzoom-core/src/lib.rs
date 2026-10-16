//! Platform-independent core of rustzoom.
//!
//! Everything that can be tested without Windows lives here: configuration,
//! the hotkey/zoom state machine and the geometry of the magnified region.
//! This crate must not depend on any Windows API.
