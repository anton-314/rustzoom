# ADR 0001: Tech stack and zoom approach

Status: accepted (2026-10-16). Settings-process and tray parts superseded by ADR 0003.

## Context

rustzoom magnifies the center of the screen while playing Rust (Facepunch) on a hotkey.
Rust is protected by Easy Anti-Cheat (EAC). Target platform: Windows 11 only. The tool is public.

## Decisions

### External screen magnifier, never touch the game
The zoom is an overlay that magnifies what is already on the screen. Hard rules, no exceptions:
- never open a handle to the game process, read/write its memory, inject DLLs or hook its rendering
- never synthesize input (no `SendInput`, no key remapping into the game)
- input is only observed passively (Raw Input with `RIDEV_INPUTSINK`), no low-level hooks

The game must run in **borderless windowed** mode; overlays are not shown over exclusive fullscreen.
Server rules about zoom tools are the player's responsibility; the README states this.

### Capture: Windows.Graphics.Capture (monitor, not game window)
- GPU-only path: captured frame stays a D3D11 texture, the center crop is scaled into the overlay swapchain
- `IsBorderRequired = false`, `IsCursorCaptureEnabled = false`
- fallback if needed: DXGI Desktop Duplication

### Overlay window
- `WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` (click-through, never takes focus)
- `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, otherwise the overlay captures itself (feedback loop).
  Side effect: the zoom is invisible in OBS/recordings.
- D3D11 + DirectComposition flip-model swapchain so the scope can be round/transparent

### Language and crates
- Rust stable, edition 2024, the official `windows` crate for all Win32/WinRT APIs
- Workspace split: `rustzoom-core` (platform-independent, unit-tested anywhere) and `rustzoom` (Windows binary)
- Settings GUI: `egui`/`eframe`, run as a **separate process** (`rustzoom.exe settings`). It only edits the
  config file; the main process watches the file and reloads it. This keeps the GUI event loop out of the
  latency-critical capture/render path.
- Config: TOML in `%APPDATA%\rustzoom\config.toml` (serde)
- Tray icon: `tray-icon` crate
- Logging: `tracing`, file in `%LOCALAPPDATA%\rustzoom\`
- Distribution: single portable `rustzoom.exe` (static CRT), GitHub Releases built by Actions

### Hotkeys
Bindable to keyboard keys and mouse buttons. Modes: hold-to-zoom and toggle, multiple zoom levels
(cycle key and/or mouse wheel while zoomed). All configurable.
