# 0001: MVP – hold mouse button 4 to show a round zoom scope

Status: in progress

## Goal
While mouse button 4 (XButton1) is held, a round scope in the center of the primary monitor shows the
screen center magnified 3×. Releasing the button hides it. The user can run this on Windows 11 and in Rust
(borderless windowed) to validate the whole approach: capture, overlay, latency, anti-cheat safety.

## Approach
- **Out of scope for this plan:** config file, settings GUI, tray icon, toggle mode/levels in the app, multi-monitor.
  The state machine in `rustzoom-core` already supports hold/toggle/levels, so later plans only wire them up.
- **Fixed MVP defaults** (from `Config::default()` in core): hotkey XButton1, hold mode, factor 3.0,
  scope diameter 40 % of the monitor height, 2 px border.
- **Process setup:** `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` (otherwise coordinates are scaled
  wrong on 125 %/150 % displays), single instance via named mutex, `tracing` logs to
  `%LOCALAPPDATA%\rustzoom\rustzoom.log`.
- **Overlay window:** `WS_POPUP`, `WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE |
  WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP`, sized to the scope square, centered on the primary monitor.
  `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` so it doesn't capture itself. Shown with
  `SW_SHOWNOACTIVATE`, hidden with `SW_HIDE`; it never takes focus.
- **Input:** Raw Input for mouse and keyboard, `RIDEV_INPUTSINK` on the overlay window (receives input while
  the game has focus). Purely passive, nothing is blocked or injected.
- **Capture:** Windows.Graphics.Capture on the primary monitor (`CreateForMonitor` via
  `IGraphicsCaptureItemInterop`), free-threaded frame pool, BGRA8. `IsCursorCaptureEnabled = false`,
  `IsBorderRequired = false` (may be ignored for an unpackaged app → see manual test 7). The session runs
  for the whole lifetime so the first zoom frame has no startup delay.
- **Rendering:** see ADR 0002. D3D11 device → Direct2D device context → DirectComposition swapchain
  (premultiplied alpha). Per frame: clear transparent, clip to an ellipse, `DrawBitmap` of the source crop
  scaled to the scope size (linear interpolation), draw the border. Only rendered while zoom is active.
- **Exit:** `Ctrl+Alt+End` quits (no tray yet). Debug builds also have a console window.
- **Anti-cheat check (ADR 0001):** no game process access, no hooks, no input injection. ✔

## Steps
- [x] 1. `rustzoom-core::geometry`: scope rect centered on a monitor rect, source crop rect for a factor,
       rounding to whole pixels.
- [x] 2. `rustzoom-core::zoom`: state machine `ZoomController` (modes Hold/Toggle, levels, `on_press`/`on_release`/
       `cycle_level` → `ZoomState { active, factor }`), `Config` struct with MVP defaults, `Hotkey` enum
       (keyboard VK or mouse button).
- [x] 3. App bootstrap: deps (`windows`, `tracing`, `tracing-subscriber`, `tracing-appender`), DPI awareness,
       single instance, logging, overlay window with all styles and display affinity, message loop,
       `Ctrl+Alt+End` exits.
- [x] 4. Raw Input → `ZoomController` → overlay show/hide. Overlay is drawn as a solid semi-transparent circle
       (no capture yet) so input and window behavior can be tested in isolation.
- [x] 5. Capture + D2D rendering of the magnified crop into the round scope.
- [x] 6. README: "Trying the MVP" section (build, run, hotkeys, borderless requirement, log location).

Each step ends with a green `/verify` (including the xwin build) and one commit. After steps 4 and 5 the
user gets a test build.

## Automated tests
`rustzoom-core`, run with `cargo test -p rustzoom-core`:
- geometry: scope centered on 1920×1080, 2560×1440, 3840×2160 and a monitor with non-zero origin
  (secondary monitor left of primary); crop size = scope size / factor; factor 1.0 → crop == scope;
  crop always inside the monitor; odd sizes round consistently (no 1 px drift).
- zoom: hold → active only between press and release; toggle → press flips, release ignored; repeated
  press events (key auto-repeat) don't flip toggle twice; cycling levels wraps around; deactivating keeps the
  selected level; `Config::default()` matches the MVP defaults.

## Manual test on Windows
Build: Claude copies the exe to `%USERPROFILE%\rustzoom\rustzoom.exe`. Run it by double-click (release) or from
a terminal (debug, with console). Expected results in *italics*.

**After step 4 (input + window, no capture):**
1. Start rustzoom. *Nothing visible, no taskbar button, no focus change. Log file exists.*
2. Start it a second time. *Second instance exits immediately, still only one process in Task Manager.*
3. On the desktop, hold mouse 4. *Semi-transparent circle in the screen center while held, gone on release.*
4. While the circle is shown, click on something under it. *Click goes through to the window below.*
5. With Notepad focused, hold mouse 4 and type. *Typing still goes to Notepad (no focus stolen).*
6. Press `Ctrl+Alt+End`. *Process exits.*

**After step 5 (full MVP):**
7. Start rustzoom on the desktop. *Note: is a yellow border around the screen visible? (expected: maybe — report it)*
8. Hold mouse 4 over text. *Round scope shows the center 3× magnified, sharp edges, thin border, no
   "hall of mirrors" (overlay doesn't show itself).*
9. Move a window under the scope while zoomed. *Zoomed image follows without visible lag.*
10. Take a screenshot (`Win+Shift+S`) while zoomed. *Scope is not in the screenshot (expected, capture exclusion).*
11. Rust → settings → display mode **Borderless**. Join a server, hold mouse 4. *Scope visible over the game,
    game keeps mouse/keyboard input, aiming still works while zoomed.*
12. Compare FPS (F1 console `perf 1`) with and without rustzoom running and while zoomed. *Report numbers.*
13. Play ~10 minutes with rustzoom running. *No EAC kick/warning.*
14. Switch Rust to **Fullscreen** (exclusive). *Scope is probably not visible – report what happens.*

## Open questions
- Does `IsBorderRequired = false` work for an unpackaged exe without the consent prompt? (manual test 7)
  If not: either accept the border, or only start the capture session while zoomed (border only during zoom,
  but ~tens of ms delay on the first frame) → decide after the test.
- Click-through with `WS_EX_LAYERED` + `WS_EX_NOREDIRECTIONBITMAP` (manual test 4); fallback: no layered
  style and `WM_NCHITTEST → HTTRANSPARENT`.
