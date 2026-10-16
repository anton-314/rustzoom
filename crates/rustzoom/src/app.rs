//! Process setup and the main message loop.

use anyhow::{Context, Result};
use rustzoom_core::config::Config;
use rustzoom_core::geometry::scope_layout;
use tracing::info;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, VK_END,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
    WM_HOTKEY,
};

use crate::overlay::{self, Overlay};

const EXIT_HOTKEY_ID: i32 = 1;

pub fn run() -> Result<()> {
    // Physical pixels everywhere; otherwise scope and capture are misaligned at 125 %/150 % scaling.
    // SAFETY: called once at startup, before any window exists.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
        .context("SetProcessDpiAwarenessContext")?;

    let config = Config::default();
    let monitor = overlay::primary_monitor()?;
    let layout = scope_layout(monitor, config.scope_diameter, config.levels[0]);
    let overlay = Overlay::create(layout.window, Some(wndproc))?;

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

    info!(
        ?monitor,
        ?layout,
        "rustzoom {} started",
        env!("CARGO_PKG_VERSION")
    );
    run_message_loop();
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

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY if wparam.0 == EXIT_HOTKEY_ID as usize => {
            // SAFETY: called on the message-loop thread.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: default handling for everything else, arguments passed through unchanged.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
