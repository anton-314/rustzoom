//! The click-through, never-focused, capture-excluded window the scope is drawn into.

use anyhow::{Context, Result, bail};
use rustzoom_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HWND_TOPMOST, LWA_ALPHA, RegisterClassExW, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetLayeredWindowAttributes, SetWindowDisplayAffinity,
    SetWindowPos, ShowWindow, WDA_EXCLUDEFROMCAPTURE, WNDCLASSEXW, WNDPROC, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

pub struct Overlay {
    hwnd: HWND,
}

impl Overlay {
    /// Creates the (hidden) overlay window covering `rect` (virtual-screen coordinates).
    pub fn create(rect: Rect, wndproc: WNDPROC) -> Result<Self> {
        let class_name = w!("rustzoom.overlay");
        // SAFETY: querying the module handle of our own executable.
        let instance = unsafe { GetModuleHandleW(None) }.context("GetModuleHandleW")?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: wndproc,
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        // SAFETY: `class` is fully initialized and the class name is a static string.
        if unsafe { RegisterClassExW(&class) } == 0 {
            bail!("RegisterClassExW: {}", windows::core::Error::from_thread());
        }

        // SAFETY: the class was registered above; all pointers are static or null.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOREDIRECTIONBITMAP,
                class_name,
                w!("rustzoom"),
                WS_POPUP,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }
        .context("CreateWindowExW")?;
        let overlay = Self { hwnd };

        // SAFETY: `hwnd` is a valid top-level window owned by this thread.
        unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) }
            .context("SetLayeredWindowAttributes")?;
        // Without this the capture would contain the overlay itself (hall of mirrors).
        // SAFETY: as above.
        unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) }
            .context("SetWindowDisplayAffinity")?;
        Ok(overlay)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Moves/resizes the window to `rect` (virtual-screen coordinates).
    pub fn set_rect(&self, rect: Rect) {
        // SAFETY: `hwnd` is valid for the lifetime of `self`.
        let result = unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE,
            )
        };
        if let Err(err) = result {
            tracing::warn!("SetWindowPos failed: {err}");
        }
    }

    pub fn set_visible(&self, visible: bool) {
        let cmd = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
        // SAFETY: `hwnd` is valid for the lifetime of `self`. The return value is the previous
        // visibility, not an error.
        let _ = unsafe { ShowWindow(self.hwnd, cmd) };
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // SAFETY: we own the window; failure during teardown is irrelevant.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Handle and bounds of the primary monitor. Bounds are in virtual-screen coordinates (physical
/// pixels, the process is per-monitor DPI aware).
pub fn primary_monitor() -> Result<(HMONITOR, Rect)> {
    // SAFETY: plain query, MONITOR_DEFAULTTOPRIMARY always returns a valid monitor.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info.cbSize` is set as required.
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        bail!("GetMonitorInfoW failed");
    }
    let r = info.rcMonitor;
    Ok((
        monitor,
        Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top),
    ))
}
