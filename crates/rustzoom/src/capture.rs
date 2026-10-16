//! Monitor capture via Windows.Graphics.Capture.
//!
//! Frames stay on the GPU. The newest frame is kept, so the scope has something to show the moment
//! it opens, even if the screen hasn't changed since (WGC only delivers frames on change).

use anyhow::{Context, Result};
use tracing::warn;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{IInspectable, Interface, factory};

/// Posted to the overlay window whenever a new frame is available.
pub const WM_APP_FRAME: u32 = WM_APP + 1;

/// Three buffers: one held as "latest", one being written, one spare.
const BUFFERS: i32 = 3;

pub struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    frame_arrived: i64,
    latest: Option<Direct3D11CaptureFrame>,
}

impl Capture {
    /// Starts capturing `monitor` with `d3d`. Each new frame posts [`WM_APP_FRAME`] to `notify`.
    pub fn start(d3d: &ID3D11Device, monitor: HMONITOR, notify: HWND) -> Result<Self> {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: `monitor` is a valid monitor handle from MonitorFromPoint.
        let item: GraphicsCaptureItem =
            unsafe { interop.CreateForMonitor(monitor) }.context("CreateForMonitor")?;

        let dxgi: IDXGIDevice = d3d.cast()?;
        // SAFETY: wraps our valid DXGI device for WinRT.
        let inspectable: IInspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?;
        let device: IDirect3DDevice = inspectable.cast()?;

        // Free-threaded: FrameArrived fires on a worker thread, we have no DispatcherQueue.
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            BUFFERS,
            item.Size()?,
        )
        .context("CreateFreeThreaded")?;

        // HWND is not Send; the raw value is, and the window outlives the capture.
        let notify = notify.0 as isize;
        let frame_arrived = pool.FrameArrived(&TypedEventHandler::new(move |_, _| {
            // SAFETY: posting to a window owned by this process; failure only means the
            // window is already gone during shutdown.
            let _ = unsafe {
                PostMessageW(Some(HWND(notify as _)), WM_APP_FRAME, WPARAM(0), LPARAM(0))
            };
            Ok(())
        }))?;

        let session = pool.CreateCaptureSession(&item)?;
        if let Err(err) = session.SetIsCursorCaptureEnabled(false) {
            warn!("cannot hide cursor in capture: {err}");
        }
        // May be ignored by Windows for an unpackaged app (plan 0001, manual test 7).
        if let Err(err) = session.SetIsBorderRequired(false) {
            warn!("cannot disable capture border: {err}");
        }
        session.StartCapture().context("StartCapture")?;

        Ok(Self {
            pool,
            session,
            frame_arrived,
            latest: None,
        })
    }

    /// Takes all pending frames from the pool and keeps the newest one.
    pub fn update(&mut self) {
        // TryGetNextFrame returns an error (null) once the pool is empty.
        while let Ok(frame) = self.pool.TryGetNextFrame() {
            if let Some(old) = self.latest.replace(frame) {
                let _ = old.Close();
            }
        }
    }

    /// Texture of the newest frame, valid until the next [`Capture::update`].
    pub fn latest_texture(&self) -> Option<ID3D11Texture2D> {
        let surface = self.latest.as_ref()?.Surface().ok()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().ok()?;
        // SAFETY: the frame surface is backed by a D3D11 texture on our device.
        unsafe { access.GetInterface() }.ok()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.pool.RemoveFrameArrived(self.frame_arrived);
        if let Some(frame) = self.latest.take() {
            let _ = frame.Close();
        }
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
