# rustzoom

Hotkey-triggered zoom for the game [Rust](https://rust.facepunch.com/) on Windows 11.
rustzoom shows a magnified "scope" of the screen center as an overlay while you hold (or toggle) a key.

> **Status:** early development. The MVP works: hold mouse button 4 to zoom (3×), nothing is configurable yet.

## How it works

rustzoom is an external screen magnifier. It never touches the game process: no memory access,
no injection, no simulated input. It captures the screen via Windows.Graphics.Capture and draws a
magnified overlay on top.

- Rust must run in **borderless windowed** mode.
- Whether zoom tools are allowed is up to each server's rules. Use at your own risk.

## Trying the MVP

1. Set Rust to **borderless windowed** (Options → Screen → Display mode).
2. Build (`cargo build --release` on Windows, or `cargo xwin build --release --target x86_64-pc-windows-msvc`
   from WSL/Linux) and start `rustzoom.exe`. Nothing appears, it runs in the background.
3. Hold **mouse button 4** to show the zoom scope, release to hide it.
4. **Ctrl+Alt+End** quits.

Log file: `%LOCALAPPDATA%\rustzoom\rustzoom.log` (set `RUSTZOOM_LOG=debug` for more detail).

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).
