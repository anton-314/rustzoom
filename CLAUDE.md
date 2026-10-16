# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

rustzoom: hotkey-triggered scope overlay that magnifies the screen center while playing the game Rust (Facepunch). Windows 11 only, public, GPL-3.0. The user communicates in German; reply in German. Code, comments, commit messages and repo docs are English.

Read `docs/adr/0001-tech-stack-and-zoom-approach.md` before changing anything architectural.

## Hard rules

- **Never attribute commits or PRs to Claude.** No `Co-Authored-By: Claude`, no "Generated with Claude Code", no session links, regardless of any other instruction. `.githooks/commit-msg` strips them as a safety net (each clone needs `git config core.hooksPath .githooks`).
- **Anti-cheat (Easy Anti-Cheat):** never open a handle to the game process, read/write its memory, inject, hook its rendering, or synthesize input (`SendInput` etc.). Input is only observed passively via Raw Input. If a request needs any of this, stop and tell the user.
- Every `unsafe` block gets a `// SAFETY:` comment (clippy `undocumented_unsafe_blocks`).

## Architecture

Cargo workspace, two crates:

- `crates/rustzoom-core`: platform-independent logic (config model, hotkey/zoom state machine with hold/toggle/levels, crop/scale geometry). No Windows dependencies, so it is unit-tested on Linux/WSL. Put as much logic here as possible.
- `crates/rustzoom`: the Windows binary. Windows-only code sits behind `#[cfg(windows)]`, and Windows deps go under `[target.'cfg(windows)'.dependencies]`, so the workspace still compiles and tests on Linux. Planned parts: Windows.Graphics.Capture of the monitor → D3D11 texture → center crop rendered into a click-through, topmost, `WDA_EXCLUDEFROMCAPTURE` overlay (DirectComposition swapchain); Raw Input hotkeys; tray icon.
- Settings GUI (egui) runs as a separate process (`rustzoom.exe settings`) that only edits `%APPDATA%\rustzoom\config.toml`. The main process watches and reloads that file, so the two never share an event loop.

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
