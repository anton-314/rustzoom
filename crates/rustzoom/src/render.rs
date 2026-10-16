//! Direct2D drawing into a DirectComposition swapchain (ADR 0002).

use std::mem::ManuallyDrop;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_OPTIONS,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Bitmap1, ID2D1DeviceContext,
    ID2D1Factory1, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::core::Interface;
use windows_numerics::Vector2;

const TRANSPARENT: D2D1_COLOR_F = color(0.0, 0.0, 0.0, 0.0);
const BORDER: D2D1_COLOR_F = color(0.08, 0.08, 0.08, 1.0);
const TEST_FILL: D2D1_COLOR_F = color(1.0, 1.0, 1.0, 0.35);

pub struct Renderer {
    size: f32,
    context: ID2D1DeviceContext,
    swapchain: IDXGISwapChain1,
    border: ID2D1SolidColorBrush,
    test_fill: ID2D1SolidColorBrush,
    // Must stay alive for the swapchain to be shown in the window.
    _composition: (
        IDCompositionDevice,
        IDCompositionTarget,
        IDCompositionVisual,
    ),
    _target: ID2D1Bitmap1,
    _d3d: ID3D11Device,
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

        // With the flip model, buffer 0 is always the current back buffer, so one target bitmap
        // is enough.
        let target_props = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            colorContext: ManuallyDrop::new(None),
        };
        // SAFETY: see above.
        let target = unsafe {
            let surface: IDXGISurface = swapchain.GetBuffer(0)?;
            let target = context.CreateBitmapFromDxgiSurface(&surface, Some(&target_props))?;
            context.SetTarget(&target);
            target
        };

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
        let (border, test_fill) = unsafe {
            (
                context.CreateSolidColorBrush(&BORDER, None)?,
                context.CreateSolidColorBrush(&TEST_FILL, None)?,
            )
        };

        Ok(Self {
            size: size as f32,
            context,
            swapchain,
            border,
            test_fill,
            _composition: composition,
            _target: target,
            _d3d: d3d,
        })
    }

    /// Placeholder until capture exists: a semi-transparent disc with the scope border.
    pub fn draw_test_circle(&self, border_width: f32) -> Result<()> {
        let ellipse = self.scope_ellipse(border_width);
        // SAFETY: drawing on our own device context between BeginDraw/EndDraw.
        unsafe {
            self.context.BeginDraw();
            self.context.Clear(Some(&TRANSPARENT));
            self.context.FillEllipse(&ellipse, &self.test_fill);
            self.context
                .DrawEllipse(&ellipse, &self.border, border_width, None);
            self.context.EndDraw(None, None)?;
        }
        self.present()
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
        // SAFETY: presenting our own swapchain.
        unsafe { self.swapchain.Present(1, DXGI_PRESENT(0)) }
            .ok()
            .context("Present")
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
