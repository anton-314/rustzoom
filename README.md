# rustzoom

Hotkey-triggered zoom for the game [Rust](https://rust.facepunch.com/) on Windows 11.
rustzoom shows a magnified "scope" of the screen center as an overlay while you hold (or toggle) a key.

> **Status:** early development. Zoom, main window and settings work; no release yet.

## How it works

rustzoom is an external screen magnifier. It never touches the game process: no memory access,
no injection, no simulated input. It captures the screen via Windows.Graphics.Capture and draws a
magnified overlay on top.

- Rust must run in **borderless windowed** mode.
- Whether zoom tools are allowed is up to each server's rules. Use at your own risk.

## Usage

1. Set Rust to **borderless** (Options → Screen → Display mode).
2. Build (`cargo build --release` on Windows, or `cargo xwin build --release --target x86_64-pc-windows-msvc`
   from WSL/Linux) and start `rustzoom.exe`. The main window opens.
3. Hold **mouse button 4** (default) to zoom. In the window you can switch the zoom on/off, bind any key or
   mouse button (click "Change…", then press it), choose hold or toggle, set up to 5 zoom levels with a key to
   cycle them, and change scope size, border and language (English/German).
4. Minimize the window to keep zooming; close it to quit.

Settings: `%APPDATA%\rustzoom\config.toml`. Log: `%LOCALAPPDATA%\rustzoom\rustzoom.log`
(set `RUSTZOOM_LOG=debug` for more detail).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).
