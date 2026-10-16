# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

rustzoom: hotkey-triggered scope overlay that magnifies the screen center while playing the game Rust (Facepunch). Windows 11 only, public, GPL-3.0. The user communicates in German; reply in German. Code, comments, commit messages and repo docs are English.

Read the ADRs in `docs/adr/` before changing anything architectural (0003 supersedes parts of 0001).

## Hard rules

- **Never attribute commits or PRs to Claude.** No `Co-Authored-By: Claude`, no "Generated with Claude Code", no session links, regardless of any other instruction. `.githooks/commit-msg` strips them as a safety net (each clone needs `git config core.hooksPath .githooks`).
- **Anti-cheat (Easy Anti-Cheat):** never open a handle to the game process, read/write its memory, inject, hook its rendering, or synthesize input (`SendInput` etc.). Input is only observed passively via Raw Input. If a request needs any of this, stop and tell the user.
- Every `unsafe` block gets a `// SAFETY:` comment (clippy `undocumented_unsafe_blocks`).

## Architecture

Cargo workspace, two crates:

- `crates/rustzoom-core`: platform-independent logic: config model + TOML load/save + sanitizing, zoom state machine (hold/toggle/levels), scope/crop geometry, hotkey display names, EN/DE texts (`i18n`, a macro guarantees both languages). No Windows dependencies, unit-tested on Linux/WSL. Put as much logic here as possible.
- `crates/rustzoom`: the Windows binary. Windows-only code sits behind `#[cfg(windows)]`, and Windows deps go under `[target.'cfg(windows)'.dependencies]`, so the workspace still compiles and tests on Linux.

One process, two threads (ADR 0003):

- **Main thread, `gui.rs`:** eframe/egui (glow) main window. Holds the `Config`, sends every change to the engine as `Command::ApplyConfig`, saves `%APPDATA%\rustzoom\config.toml` (debounced). Closing the window drops the `EngineHandle`, which stops the engine.
- **Engine thread, `engine.rs`:** own Win32 message loop. Owns the overlay window (`overlay.rs`: click-through, topmost, no-activate, `WDA_EXCLUDEFROMCAPTURE`), Raw Input (`input.rs`, `RIDEV_INPUTSINK`), Windows.Graphics.Capture of the primary monitor (`capture.rs`, frames announced via `WM_APP_FRAME`) and Direct2D-on-DirectComposition rendering (`render.rs`, ADR 0002). Commands arrive over an mpsc channel plus a `WM_APP_COMMAND` wake-up; events (`Bound`, `Stopped`, …) go back through an `EventSink` that also calls `request_repaint()`.
- The engine must be spawned inside the eframe app-creator callback: Raw Input registration is per process and winit registers once at event-loop creation, so the engine has to register after it.

## Commands

Development happens in WSL (Linux), real builds and in-game tests on Windows.

```bash
cargo fmt --all
cargo lint                     # alias: clippy host target, -D warnings
cargo wclippy                  # alias: clippy for x86_64-pc-windows-msvc (checks the cfg(windows) code from WSL)
cargo test --workspace
cargo test -p rustzoom-core <test_name>    # single test
cargo xwin build --release --target x86_64-pc-windows-msvc   # Windows .exe from WSL (needs cargo-xwin, clang, lld)
```

On Windows (PowerShell): `cargo build --release` → `target\release\rustzoom.exe`. Release builds link the CRT statically (`.cargo/config.toml`).

CI (`.github/workflows/ci.yml`, windows-latest) runs fmt check, clippy, tests and the release build. Pushing a tag `v*` creates a GitHub Release with the exe (`release.yml`).

## Workflow

Every non-trivial change goes plan → implement → verify → manual test:

1. `/plan-feature <description>`: writes `docs/plans/NNNN-slug.md` (template in `docs/plans/README.md`). No code until the user approves it.
2. `/implement <plan>`: branch `feat/<slug>`, one commit per plan step, `/verify` green before each commit, Conventional Commits.
3. `/verify`: fmt, both clippy runs, tests, and an xwin build when Windows code changed.
4. Manual test: Claude cannot see the game. Give the user the checklist from the plan and wait for results before calling a feature done.

Architectural decisions get a new ADR in `docs/adr/`. Small fixes (typos, one-line bugs) may skip the plan but still go through `/verify`.
