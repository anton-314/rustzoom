//! Process setup, app state and the main message loop.

use std::cell::RefCell;

use anyhow::{Context, Result};
use rustzoom_core::config::Config;
use rustzoom_core::geometry::scope_layout;
use rustzoom_core::zoom::{ZoomController, ZoomState};
use tracing::{debug, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, VK_END,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
    WM_HOTKEY, WM_INPUT,
};

use crate::input::{self, InputEvent};
use crate::overlay::{self, Overlay};
use crate::render::Renderer;

const EXIT_HOTKEY_ID: i32 = 1;

thread_local! {
    /// App state for the window procedure. Only touched on the message-loop thread.
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

struct App {
    config: Config,
    zoom: ZoomController,
    overlay: Overlay,
    renderer: Renderer,
    visible: bool,
}

impl App {
    fn on_input(&mut self, event: InputEvent) {
        let state = match event {
            InputEvent::Pressed(key) if key == self.config.hotkey => self.zoom.on_press(),
            InputEvent::Released(key) if key == self.config.hotkey => self.zoom.on_release(),
            _ => return,
        };
        self.apply(state);
    }

    fn apply(&mut self, state: ZoomState) {
        if state.active == self.visible {
            return;
        }
        debug!(?state, "zoom changed");
        if state.active {
            // Draw before showing, so the window never shows a stale frame.
            if let Err(err) = self.renderer.draw_test_circle(self.config.border_width) {
                warn!("drawing failed: {err:#}");
                return;
            }
        }
        self.overlay.set_visible(state.active);
        self.visible = state.active;
    }
}

pub fn run() -> Result<()> {
    // Physical pixels everywhere; otherwise scope and capture are misaligned at 125 %/150 % scaling.
    // SAFETY: called once at startup, before any window exists.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        .context("SetProcessDpiAwarenessContext")?;

    let config = Config::default();
    let monitor = overlay::primary_monitor()?;
    let layout = scope_layout(monitor, config.scope_diameter, config.levels[0]);
    let overlay = Overlay::create(layout.window, Some(wndproc))?;
    let renderer = Renderer::new(overlay.hwnd(), layout.window.width)?;

    // SAFETY: the window belongs to this thread; the id is unique within it.
    unsafe {
        RegisterHotKey(
            Some(overlay.hwnd()),
            EXIT_HOTKEY_ID,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            u32::from(VK_END.0),
        )
    }
    .context("RegisterHotKey Ctrl+Alt+End")?;
    input::register(overlay.hwnd())?;

    info!(
        ?monitor,
        ?layout,
        "rustzoom {} started",
        env!("CARGO_PKG_VERSION")
    );
    let zoom = ZoomController::new(config.mode, config.levels.clone());
    APP.set(Some(App {
        config,
        zoom,
        overlay,
        renderer,
        visible: false,
    }));
    run_message_loop();
    // Tear down windows and GPU objects while the thread is still fully alive.
    APP.take();
    info!("rustzoom exiting");
    Ok(())
}

fn run_message_loop() {
    let mut msg = MSG::default();
    // SAFETY: standard message loop on the thread that owns the window. GetMessageW returns -1 on
    // error, so loop only while it is > 0.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        // SAFETY: `msg` was filled by GetMessageW.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Runs `f` on the app state. Skips silently if the state is not set up yet or already borrowed
/// (re-entrant window messages sent while handling another one).
fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Ok(mut app) = cell.try_borrow_mut()
            && let Some(app) = app.as_mut()
        {
            f(app);
        }
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            for event in input::read(lparam) {
                with_app(|app| app.on_input(event));
            }
            // DefWindowProc must still see WM_INPUT to free the raw input buffer.
            // SAFETY: arguments passed through unchanged.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_HOTKEY if wparam.0 == EXIT_HOTKEY_ID as usize => {
            // SAFETY: called on the message-loop thread.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default handling for everything else, arguments passed through unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
