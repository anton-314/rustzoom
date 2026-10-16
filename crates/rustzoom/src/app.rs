//! Process setup, app state and the main message loop.

use std::cell::RefCell;

use anyhow::{Context, Result};
use rustzoom_core::config::Config;
use rustzoom_core::geometry::{Rect, scope_layout};
use rustzoom_core::zoom::{ZoomController, ZoomState};
use tracing::{debug, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize};
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

use crate::capture::{Capture, WM_APP_FRAME};
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
    monitor: Rect,
    zoom: ZoomController,
    overlay: Overlay,
    renderer: Renderer,
    capture: Capture,
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

    fn on_frame(&mut self) {
        self.capture.update();
        let state = self.zoom.state();
        if state.active {
            self.draw(state);
        }
    }

    fn apply(&mut self, state: ZoomState) {
        debug!(?state, "zoom changed");
        if state.active {
            // Draw before showing, so the window never shows a stale image.
            self.draw(state);
        }
        self.overlay.set_visible(state.active);
    }

    fn draw(&self, state: ZoomState) {
        let source = scope_layout(self.monitor, self.config.scope_diameter, state.factor).source;
        let frame = self.capture.latest_texture();
        if let Err(err) = self
            .renderer
            .draw(frame.as_ref(), source, self.config.border_width)
        {
            warn!("drawing failed: {err:#}");
        }
    }
}

pub fn run() -> Result<()> {
    // Physical pixels everywhere; otherwise scope and capture are misaligned at 125 %/150 % scaling.
    // SAFETY: called once at startup, before any window exists.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        .context("SetProcessDpiAwarenessContext")?;
    // WinRT (Windows.Graphics.Capture) on this thread.
    // SAFETY: once per thread, before any WinRT call.
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.context("RoInitialize")?;

    let config = Config::default();
    let (hmonitor, monitor) = overlay::primary_monitor()?;
    let layout = scope_layout(monitor, config.scope_diameter, config.levels[0]);
    let overlay = Overlay::create(layout.window, Some(wndproc))?;
    let renderer = Renderer::new(overlay.hwnd(), layout.window.width)?;
    let capture = Capture::start(renderer.device(), hmonitor, overlay.hwnd())?;

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
        monitor,
        zoom,
        overlay,
        renderer,
        capture,
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
        WM_APP_FRAME => {
            with_app(App::on_frame);
            LRESULT(0)
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
