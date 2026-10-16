# ADR 0002: Rendering with Direct2D on DirectComposition

Status: accepted (2026-10-16)

## Context
The scope must scale a crop of the captured monitor texture into a round, transparent overlay with low
latency. ADR 0001 fixed D3D11 + DirectComposition but not how the scaling/masking is drawn.

## Decision
Draw with **Direct2D** (`ID2D1DeviceContext`) on top of the shared D3D11 device, presenting through a
**DirectComposition** swapchain (`CreateSwapChainForComposition`, flip model, premultiplied alpha).

- The captured `ID3D11Texture2D` is wrapped as an `ID2D1Bitmap1` via its DXGI surface: no CPU copy.
- Scaling: `DrawBitmap` with source/destination rects and linear interpolation.
- Round shape and border: ellipse geometry clip (`PushLayer`) plus `DrawEllipse`.

## Consequences
- No HLSL shaders, no runtime shader compiler, much less code than a hand-written D3D11 pipeline.
- Direct2D is single-threaded per device context; all drawing happens on one thread.
- If profiling later shows D2D overhead matters, the pipeline can be replaced by a D3D11 pixel shader
  without changing capture or window code.
