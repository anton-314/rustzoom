# 0002: Main window with on/off and settings

Status: done

## Goal
rustzoom opens a main window where the user can switch the zoom on/off and configure it: zoom hotkey
(any key or mouse button), hold/toggle mode, zoom levels with a key to cycle them, scope size, border width,
and the UI language (English/German). Settings apply immediately and are saved.

## Approach
- Architecture per **ADR 0003**: GUI (eframe/egui) on the main thread, the current zoom logic becomes the
  *engine* on its own thread. Closing the window quits; minimizing keeps zooming.
- **Config file** `%APPDATA%\rustzoom\config.toml` (serde + toml, in `rustzoom-core`). Missing file → defaults;
  unreadable file → defaults, the broken file is kept as `config.toml.bak`, warning in the log. Written
  atomically (temp file + rename), at most every 500 ms while values change and once on exit.
- **New config fields:** `enabled`, `language`, `cycle_hotkey` (optional). All values are sanitized on load
  and from the GUI: 1–5 levels, each 1.5×–10×; diameter 10–90 % of monitor height; border 0–10 px.
- **Default language:** German if the Windows UI language is German, otherwise English.
- **Translations** live in `rustzoom-core::i18n` as a `match` over a `Text` enum, so the compiler enforces
  that every text exists in both languages.
- **Key binding:** "Change…" puts the engine into binding mode; the next key/mouse press (via Raw Input,
  so mouse 4/5 and Alt/Shift/Ctrl work) becomes the hotkey. `Esc` cancels. The left mouse button can't be
  bound (it would make the GUI unusable). Names for display ("Mouse 4", "Left Alt" → "Alt", "F") come from
  a VK-code table in core.
- **Engine reacts live:** size change → overlay moved/resized and swapchain `ResizeBuffers`;
  on/off → when off the capture session is stopped (no GPU cost) and the hotkey ignored.
- **Preview:** after changing size, border or a level, the engine shows the scope for 1.5 s.
- `Ctrl+Alt+End` is removed; the window is the way to quit.
- **Anti-cheat check (ADR 0001):** unchanged, binding mode only reads Raw Input. ✔

## Steps
- [x] 1. core: serde for `Config` (+ `enabled`, `language`, `cycle_hotkey`), `Config::sanitized()`,
       TOML load/save functions (path passed in, atomic write, `.bak` on parse error), VK/mouse display names.
- [x] 2. core: `i18n` with `Language` and `Text`, `text(lang, Text)`.
- [x] 3. Engine on its own thread: move app state into `engine.rs`, command channel (`ApplyConfig`,
       `SetEnabled`, `Quit`) with wake-up message, cycle-level hotkey, stop/start capture on off/on, live
       resize. `main` still has no GUI: loads `config.toml`, runs the engine, so it can be tested by editing the file.
- [x] 4. Main window (eframe): on/off switch, mode, levels (add/remove, sliders), size, border, language;
       applies via commands, saves config, closing quits. Engine started from the app-creator callback.
- [x] 5. Key binding for zoom and cycle hotkey (binding mode in engine, events back to GUI), preview after changes.
- [x] 6. Docs: README (usage), CLAUDE.md architecture section, plan results.

Each step: `/verify` green incl. xwin build, one commit. Test builds for the user after steps 3, 4 and 5.

## Automated tests
`cargo test -p rustzoom-core`:
- config: TOML roundtrip of default and a fully custom config; missing fields fall back to defaults (old
  files keep working); `sanitized()` clamps every field, drops extra levels, replaces an empty level list,
  rejects NaN; load from missing file → default; load from garbage → default + `.bak` created;
  save is atomic (no temp file left, content readable).
- names: mouse buttons, letters, digits, F-keys, Alt/Shift/Ctrl, unknown code → "Key 0x…".
- i18n: every `Text` is non-empty in both languages; English and German differ for every text except
  explicit exceptions (e.g. "rustzoom").

## Manual test on Windows
Expected results in *italics*. Carried over from plan 0001: also check 9–14 there (in-game, FPS, EAC, fullscreen).

**After step 3 (engine, no window):**
1. Start without `config.toml`. *Behaves like the MVP; `%APPDATA%\rustzoom\config.toml` is not required.*
2. Create `config.toml` with `scope_diameter = 0.6` and `levels = [2.0, 4.0]`, `cycle_hotkey = { mouse = "x2" }`,
   restart. *Bigger scope; mouse 5 switches 2× ↔ 4×.*
3. Write garbage into `config.toml`, restart. *Defaults are used, `config.toml.bak` exists, warning in log.*

**After step 4 (window):**
4. Start rustzoom. *Main window opens in the system language; zoom works while it's open and when minimized.*
5. Switch off. *Mouse 4 does nothing.* Switch on. *Works again.*
6. Change size, border, levels and mode (toggle). *Effect is immediate; restart keeps the values.*
7. Switch language. *All texts switch at once; persists after restart.*
8. Close the window. *Process is gone (Task Manager).*

**After step 5 (binding, preview):**
9. "Change…" for the zoom key, press `Alt`. *Shows "Alt"; holding Alt zooms.* Repeat with mouse 5 and `F`.
10. "Change…", press `Esc`. *Binding unchanged.*
11. Drag the size slider. *Scope preview appears in the screen center and disappears ~1.5 s after letting go.*
12. In Rust (borderless): zoom, cycle levels, game input unaffected.

## Results
- 2026-10-16, user on Windows 11: "everything works": main window, on/off, settings apply live and persist,
  key binding (incl. Esc cancel), preview, language switch, clean exit.
- FPS numbers, a longer EAC session and exclusive fullscreen were not reported separately; still worth checking.

## Open questions
- Window icon: not part of this plan (needs an icon asset), currently the default icon.
