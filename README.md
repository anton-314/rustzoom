# rustzoom

Hotkey-triggered zoom for the game [Rust](https://rust.facepunch.com/) on Windows 11.
rustzoom shows a magnified "scope" of the screen center as an overlay while you hold (or toggle) a key.

> **Status:** early development, nothing usable yet.

## How it works

rustzoom is an external screen magnifier. It never touches the game process: no memory access,
no injection, no simulated input. It captures the screen via Windows.Graphics.Capture and draws a
magnified overlay on top.

- Rust must run in **borderless windowed** mode.
- Whether zoom tools are allowed is up to each server's rules. Use at your own risk.

## License

GPL-3.0-or-later, see [LICENSE](LICENSE).
