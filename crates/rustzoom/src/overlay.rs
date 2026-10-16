//! The click-through, never-focused, capture-excluded window the scope is drawn into.

use anyhow::{Context, Result, bail};
use rustzoom_core::geometry::Rect;
use windows::Win32::Foundation::{COLORREF, HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, LWA_ALPHA, RegisterClassExW, SetLayeredWindowAttributes,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WNDCLASSEXW, WNDPROC, WS_EX_LAYERED,
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
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // SAFETY: we own the window; failure during teardown is irrelevant.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

/// Bounds of the primary monitor in virtual-screen coordinates (physical pixels, the process is
/// per-monitor DPI aware).
pub fn primary_monitor() -> Result<Rect> {
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
    Ok(Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top))
}
