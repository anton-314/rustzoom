//! Where the scope sits on screen and which part of the captured monitor it shows.

/// Axis-aligned pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    const fn offset(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }
}

/// Placement of the scope for one monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeLayout {
    /// Overlay window, in virtual-screen coordinates. Always square.
    pub window: Rect,
    /// Region of the captured monitor image that gets magnified, in monitor-local coordinates
    /// (the capture texture starts at 0,0). Always square.
    pub source: Rect,
}

/// Computes the scope layout for `monitor` (virtual-screen coordinates).
///
/// `diameter_fraction` is the scope diameter relative to the monitor height, `factor` the
/// magnification. Both are clamped to sane ranges, so the result always lies inside the monitor.
pub fn scope_layout(monitor: Rect, diameter_fraction: f32, factor: f32) -> ScopeLayout {
    let max_diameter = monitor.width.min(monitor.height).max(1);
    let diameter = (monitor.height as f32 * diameter_fraction.clamp(0.0, 1.0)).round() as i32;
    let diameter = diameter.clamp(1, max_diameter);

    let source_size = (diameter as f32 / factor.max(1.0)).round() as i32;
    let source_size = source_size.clamp(1, diameter);

    ScopeLayout {
        window: centered(monitor.width, monitor.height, diameter).offset(monitor.x, monitor.y),
        source: centered(monitor.width, monitor.height, source_size),
    }
}

/// A `size`×`size` square centered in a `width`×`height` area starting at 0,0.
fn centered(width: i32, height: i32, size: i32) -> Rect {
    Rect::new((width - size) / 2, (height - size) / 2, size, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITORS: [Rect; 4] = [
        Rect::new(0, 0, 1920, 1080),
        Rect::new(0, 0, 2560, 1440),
        Rect::new(0, 0, 3840, 2160),
        // Secondary monitor left of the primary one.
        Rect::new(-1920, 0, 1920, 1080),
    ];

    fn contains(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.right() <= outer.right()
            && inner.bottom() <= outer.bottom()
    }

    /// Twice the center, to stay in integers.
    fn center2(r: Rect) -> (i32, i32) {
        (2 * r.x + r.width, 2 * r.y + r.height)
    }

    #[test]
    fn full_hd_defaults() {
        let layout = scope_layout(MONITORS[0], 0.4, 3.0);
        assert_eq!(layout.window, Rect::new(744, 324, 432, 432));
        assert_eq!(layout.source, Rect::new(888, 468, 144, 144));
    }

    #[test]
    fn window_is_centered_and_inside_monitor() {
        for monitor in MONITORS {
            let layout = scope_layout(monitor, 0.4, 3.0);
            assert!(contains(monitor, layout.window), "{monitor:?}");
            let (wx, wy) = center2(layout.window);
            let (mx, my) = center2(monitor);
            assert!((wx - mx).abs() <= 1 && (wy - my).abs() <= 1, "{monitor:?}");
        }
    }

    #[test]
    fn source_is_monitor_local_and_concentric_with_window() {
        for monitor in MONITORS {
            for factor in [1.0, 1.5, 2.0, 3.0, 4.0, 7.3] {
                let layout = scope_layout(monitor, 0.4, factor);
                let local = Rect::new(0, 0, monitor.width, monitor.height);
                assert!(contains(local, layout.source), "{monitor:?} x{factor}");

                // At most half a pixel apart, so the zoom doesn't drift sideways.
                let window_local = layout.window.offset(-monitor.x, -monitor.y);
                let (wx, wy) = center2(window_local);
                let (sx, sy) = center2(layout.source);
                assert!(
                    (wx - sx).abs() <= 1 && (wy - sy).abs() <= 1,
                    "{monitor:?} x{factor}"
                );
            }
        }
    }

    #[test]
    fn source_size_is_window_size_divided_by_factor() {
        let layout = scope_layout(MONITORS[2], 0.4, 4.0);
        assert_eq!(layout.window.width, 864);
        assert_eq!(layout.source.width, 216);
    }

    #[test]
    fn factor_one_shows_unmagnified_region() {
        let layout = scope_layout(MONITORS[1], 0.4, 1.0);
        assert_eq!(layout.source.width, layout.window.width);
    }

    #[test]
    fn out_of_range_inputs_are_clamped() {
        let monitor = MONITORS[0];
        let huge = scope_layout(monitor, 5.0, 3.0);
        assert_eq!(huge.window.width, 1080);
        assert!(contains(monitor, huge.window));

        let tiny = scope_layout(monitor, 0.0, 3.0);
        assert_eq!(tiny.window.width, 1);
        assert_eq!(tiny.source.width, 1);

        let below_one = scope_layout(monitor, 0.4, 0.5);
        assert_eq!(below_one.source.width, below_one.window.width);
    }
}
