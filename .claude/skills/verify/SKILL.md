---
name: verify
description: Run all automated checks for rustzoom (format, lints for host and Windows target, tests, optional Windows build) and report the result.
---

# Verify

Run from the repo root, in this order, and stop at the first failure to fix it:

```bash
cargo fmt --all          # then make sure nothing unintended changed
cargo lint               # clippy, host target, -D warnings
cargo wclippy            # clippy against x86_64-pc-windows-msvc: checks all #[cfg(windows)] code
cargo test --workspace
```

If the change touches Windows code and `cargo-xwin` is installed, also build the real executable:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc
# result: target/x86_64-pc-windows-msvc/release/rustzoom.exe (can be started from WSL)
```

Report to the user: which checks ran, pass/fail, and anything skipped (e.g. "Windows build skipped, cargo-xwin
missing"). Automated checks never replace the manual in-game test; say which manual checks are still open.
