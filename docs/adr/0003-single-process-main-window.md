# ADR 0003: Single process with a main window

Status: accepted (2026-10-16). Supersedes the "settings GUI as separate process" and "tray icon" parts of ADR 0001.

## Context
The user wants one main window (no tray icon) that opens on start and holds all settings, with the language
switchable between English and German. ADR 0001 planned a separate settings process talking through the
config file, to keep the GUI event loop away from the zoom path.

## Decision
One process, two threads:

- **Main thread: GUI.** `eframe`/`egui` (glow renderer) owns the main window. Closing it quits rustzoom,
  minimizing keeps it running.
- **Engine thread:** the existing Win32 message loop with overlay window, Raw Input, capture and rendering.
  It never waits on the GUI.
- **GUI → engine:** commands over an `mpsc` channel, plus a `PostMessageW` to the engine window to wake its
  `GetMessageW` loop.
- **Engine → GUI:** events (e.g. "key bound") over an `mpsc` channel, plus `egui::Context::request_repaint()`.
- **Config file stays the persistence format** (`%APPDATA%\rustzoom\config.toml`), written by the GUI.

### Raw Input ownership
Raw Input registration is per process and per device type; the last `RegisterRawInputDevices` call wins.
winit (under eframe) registers mice and keyboards once, when its event loop is created (winit 0.30
`EventLoop::new` → `register_all_mice_and_keyboards_for_raw_input`). The engine therefore starts **inside the
eframe app-creator callback**, i.e. after winit's registration, and its registration wins. egui does not need
winit's device events. Upgrading winit/eframe requires re-checking this (grep `register_all_mice_and_keyboards`).

## Consequences
- No IPC protocol, no second process to manage; key binding can use the engine's Raw Input directly
  (mouse buttons and modifiers included).
- A GUI hang can't freeze the zoom, but a panic in either thread must end the process cleanly
  (engine panics are logged and shut the GUI down).
