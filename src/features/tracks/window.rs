//! Pure math for list virtualization and keyboard reordering (no UI).

/// Fixed row height in px; every row of a `TrackList` has exactly this height.
pub const ROW_H: f64 = 56.0;
/// Lists with more rows than this render only the rows near the viewport.
pub const VIRTUAL_THRESHOLD: usize = 200;
/// Extra rows rendered above and below the viewport.
pub const OVERSCAN: usize = 8;

/// Which rows to render and how much blank space stands in for the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
    pub pad_top: f64,
    pub pad_bottom: f64,
}

/// `offset` is how many px of the list are scrolled above the viewport top (negative while the
/// list starts below it); `viewport` is the visible height. Short lists render completely.
pub fn window(len: usize, offset: f64, viewport: f64, row_h: f64) -> Window {
    let (start, end) = if len <= VIRTUAL_THRESHOLD || row_h <= 0.0 {
        (0, len)
    } else {
        let first = (offset.max(0.0) / row_h).floor() as usize;
        let last = ((offset + viewport.max(0.0)).max(0.0) / row_h).ceil() as usize;
        let start = first.saturating_sub(OVERSCAN).min(len);
        let end = (last + OVERSCAN).min(len).max(start);
        (start, end)
    };
    Window {
        start,
        end,
        pad_top: start as f64 * row_h,
        pad_bottom: (len - end) as f64 * row_h,
    }
}

/// Index a row ends up at when moved by `delta` positions, or `None` if that is out of range.
pub fn move_target(index: usize, delta: isize, len: usize) -> Option<usize> {
    let target = index.checked_add_signed(delta)?;
    (target < len && target != index).then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lists_render_everything() {
        let w = window(150, 4000.0, 600.0, ROW_H);
        assert_eq!((w.start, w.end), (0, 150));
        assert_eq!((w.pad_top, w.pad_bottom), (0.0, 0.0));
    }

    #[test]
    fn long_list_at_top_renders_viewport_plus_overscan() {
        let w = window(1000, 0.0, 560.0, ROW_H);
        assert_eq!(w.start, 0);
        assert_eq!(w.end, 10 + OVERSCAN);
        assert_eq!(w.pad_top, 0.0);
        assert_eq!(w.pad_bottom, (1000 - w.end) as f64 * ROW_H);
    }

    #[test]
    fn long_list_in_the_middle() {
        let w = window(1000, 5600.0, 560.0, ROW_H);
        assert_eq!(w.start, 100 - OVERSCAN);
        assert_eq!(w.end, 110 + OVERSCAN);
        let total = w.pad_top + (w.end - w.start) as f64 * ROW_H + w.pad_bottom;
        assert_eq!(total, 1000.0 * ROW_H);
    }

    #[test]
    fn long_list_scrolled_past_the_end_is_clamped() {
        let w = window(300, 1_000_000.0, 560.0, ROW_H);
        assert!(w.start <= w.end && w.end == 300);
        assert_eq!(w.pad_bottom, 0.0);
    }

    #[test]
    fn list_below_the_viewport_renders_only_the_first_rows() {
        let w = window(500, -2000.0, 500.0, ROW_H);
        assert_eq!(w.start, 0);
        assert_eq!(w.end, OVERSCAN);
    }

    #[test]
    fn move_target_respects_bounds() {
        assert_eq!(move_target(0, -1, 5), None);
        assert_eq!(move_target(4, 1, 5), None);
        assert_eq!(move_target(2, -1, 5), Some(1));
        assert_eq!(move_target(2, 1, 5), Some(3));
        assert_eq!(move_target(2, 0, 5), None);
    }
}
