//! Direct2D drawing into a DirectComposition swapchain (ADR 0002).

use std::mem::ManuallyDrop;

use anyhow::{Context, Result};
use rustzoom_core::geometry::Rect;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_BRUSH_PROPERTIES1, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_BRUSH_PROPERTIES,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_INTERPOLATION_MODE_LINEAR, D2D1CreateFactory,
    ID2D1Bitmap1, ID2D1BitmapBrush1, ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::core::Interface;
use windows_numerics::{Matrix3x2, Vector2};

const TRANSPARENT: D2D1_COLOR_F = color(0.0, 0.0, 0.0, 0.0);
const BORDER: D2D1_COLOR_F = color(0.08, 0.08, 0.08, 1.0);

pub struct Renderer {
    size: f32,
    context: ID2D1DeviceContext,
    swapchain: IDXGISwapChain1,
    border: ID2D1SolidColorBrush,
    // Must stay alive for the swapchain to be shown in the window.
    _composition: (
        IDCompositionDevice,
        IDCompositionTarget,
        IDCompositionVisual,
    ),
    target: Option<ID2D1Bitmap1>,
    d3d: ID3D11Device,
}

impl Renderer {
    /// Sets up a `size`×`size` transparent drawing surface covering the client area of `hwnd`.
    pub fn new(hwnd: HWND, size: i32) -> Result<Self> {
        let d3d = create_d3d_device()?;
        let dxgi: IDXGIDevice = d3d.cast()?;

        // All unsafe blocks in this function are COM calls on freshly created, valid interfaces
        // with fully initialized descriptor structs.
        // SAFETY: see above.
        let factory: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent()? };
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: size as u32,
            Height: size as u32,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            ..Default::default()
        };
        // SAFETY: see above.
        let swapchain = unsafe { factory.CreateSwapChainForComposition(&dxgi, &desc, None) }
            .context("CreateSwapChainForComposition")?;

        // SAFETY: see above.
        let context = unsafe {
            let d2d: ID2D1Factory1 = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                Some(&D2D1_FACTORY_OPTIONS::default()),
            )?;
            d2d.CreateDevice(&dxgi)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?
        };

        let target = create_target(&context, &swapchain)?;

        // SAFETY: see above; `hwnd` is a valid window of this process.
        let composition = unsafe {
            let device: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let comp_target = device.CreateTargetForHwnd(hwnd, true)?;
            let visual = device.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            comp_target.SetRoot(&visual)?;
            device.Commit()?;
            (device, comp_target, visual)
        };

        // SAFETY: see above.
        let border = unsafe { context.CreateSolidColorBrush(&BORDER, None)? };

        Ok(Self {
            size: size as f32,
            context,
            swapchain,
            border,
            _composition: composition,
            target: Some(target),
            d3d,
        })
    }

    /// Changes the surface to `size`×`size` pixels.
    pub fn resize(&mut self, size: i32) -> Result<()> {
        // The swapchain buffers can only be resized once nothing references them.
        // SAFETY: our own context; unsetting the target releases its reference to the buffer.
        unsafe { self.context.SetTarget(None) };
        self.target = None;
        // SAFETY: no outstanding references to the buffers (see above).
        unsafe {
            self.swapchain.ResizeBuffers(
                0,
                size as u32,
                size as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        }
        .context("ResizeBuffers")?;
        self.target = Some(create_target(&self.context, &self.swapchain)?);
        self.size = size as f32;
        Ok(())
    }

    /// The device capture textures must come from.
    pub fn device(&self) -> &ID3D11Device {
        &self.d3d
    }

    /// Draws `source` (pixels of `frame`) scaled into the round scope, plus the border.
    /// Without a frame only the border is drawn.
    pub fn draw(
        &self,
        frame: Option<&ID3D11Texture2D>,
        source: Rect,
        border_width: f32,
    ) -> Result<()> {
        let ellipse = self.scope_ellipse(border_width);
        let fill = frame
            .map(|frame| self.zoom_brush(frame, source))
            .transpose()?;
        // SAFETY: drawing on our own device context between BeginDraw/EndDraw.
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&TRANSPARENT));
            if let Some(fill) = &fill {
                self.context.FillEllipse(&ellipse, fill);
            }
            self.context
                .DrawEllipse(&ellipse, &self.border, border_width, None);
            self.context.EndDraw(None, None)?;
        }
        self.present()
    }

    /// A brush that paints `frame` so that `source` exactly covers the scope surface.
    fn zoom_brush(&self, frame: &ID3D11Texture2D, source: Rect) -> Result<ID2D1BitmapBrush1> {
        let scale = self.size / source.width as f32;
        let bitmap_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
            colorContext: ManuallyDrop::new(None),
        };
        let brush_props = D2D1_BITMAP_BRUSH_PROPERTIES1 {
            extendModeX: D2D1_EXTEND_MODE_CLAMP,
            extendModeY: D2D1_EXTEND_MODE_CLAMP,
            interpolationMode: D2D1_INTERPOLATION_MODE_LINEAR,
        };
        let transform = D2D1_BRUSH_PROPERTIES {
            opacity: 1.0,
            transform: Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: -source.x as f32 * scale,
                M32: -source.y as f32 * scale,
            },
        };
        let surface: IDXGISurface = frame.cast()?;
        // SAFETY: wraps the capture texture (same device) without copying; the bitmap and brush
        // only live for this draw call.
        unsafe {
            let bitmap = self
                .context
                .CreateBitmapFromDxgiSurface(&surface, Some(&bitmap_props))?;
            Ok(self
                .context
                .CreateBitmapBrush(&bitmap, Some(&brush_props), Some(&transform))?)
        }
    }

    /// The scope circle, inset so the border stays inside the surface.
    fn scope_ellipse(&self, border_width: f32) -> D2D1_ELLIPSE {
        let half = self.size / 2.0;
        let radius = half - border_width / 2.0;
        D2D1_ELLIPSE {
            point: Vector2 { X: half, Y: half },
            radiusX: radius,
            radiusY: radius,
        }
    }

    fn present(&self) -> Result<()> {
        // Sync interval 0: no vsync wait in the message loop. The flip model + DWM composition
        // never tears.
        // SAFETY: presenting our own swapchain.
        unsafe { self.swapchain.Present(0, DXGI_PRESENT(0)) }
            .ok()
            .context("Present")
    }
}

/// Wraps the swapchain's back buffer as Direct2D target and sets it on `context`. With the flip
/// model, buffer 0 is always the current back buffer, so one target bitmap is enough.
fn create_target(
    context: &ID2D1DeviceContext,
    swapchain: &IDXGISwapChain1,
) -> Result<ID2D1Bitmap1> {
    let props = D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: ManuallyDrop::new(None),
    };
    // SAFETY: buffer 0 of a valid swapchain, wrapped on the same device.
    unsafe {
        let surface: IDXGISurface = swapchain.GetBuffer(0)?;
        let target = context.CreateBitmapFromDxgiSurface(&surface, Some(&props))?;
        context.SetTarget(&target);
        Ok(target)
    }
}

fn create_d3d_device() -> Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: out-pointer to a local Option; BGRA support is required by Direct2D.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }
    .context("D3D11CreateDevice")?;
    device.context("D3D11CreateDevice returned no device")
}

const fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}
