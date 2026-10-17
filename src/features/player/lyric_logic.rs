//! Pure rules for synced lyrics: which line is active and when auto-scroll may move the view.

use crate::model::LyricLine;

/// Lines light up a little before their stamp so the highlight feels in time with the voice.
pub const LEAD_MS: u64 = 150;
/// After the user scrolls by hand, auto-scroll stays out of the way for this long.
pub const MANUAL_SCROLL_PAUSE_MS: u64 = 4_000;

/// Index of the line to highlight at `position_secs` (`None` before the first line).
pub fn active_line(lines: &[LyricLine], position_secs: f64) -> Option<usize> {
    if !position_secs.is_finite() || position_secs < 0.0 {
        return None;
    }
    let ms = (position_secs * 1000.0) as u64 + LEAD_MS;
    lines.partition_point(|l| l.start_ms <= ms).checked_sub(1)
}

/// Whether auto-scroll may run: not within the pause window after a manual scroll.
pub fn should_autoscroll(now_ms: u64, last_manual_ms: Option<u64>) -> bool {
    last_manual_ms.is_none_or(|t| now_ms.saturating_sub(t) >= MANUAL_SCROLL_PAUSE_MS)
}

/// Opacity of line `i` by its distance to the active line: 1, .5, .28, then .2 (the lines fade
/// out like type on a stage). Before the first line the first one counts as "next".
pub fn line_opacity(active: Option<usize>, i: usize) -> f32 {
    let a = active.map_or(-1, |a| a as isize);
    match (i as isize - a).unsigned_abs() {
        0 => 1.0,
        1 => 0.5,
        2 => 0.28,
        _ => 0.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(starts: &[u64]) -> Vec<LyricLine> {
        starts
            .iter()
            .map(|&s| LyricLine {
                start_ms: s,
                text: format!("line {s}"),
                words: vec![],
            })
            .collect()
    }

    #[test]
    fn before_first_line_nothing_is_active() {
        let l = lines(&[5_000, 9_000]);
        assert_eq!(active_line(&l, 0.0), None);
        assert_eq!(active_line(&l, 4.8), None);
    }

    #[test]
    fn line_turns_active_slightly_early() {
        let l = lines(&[5_000, 9_000]);
        assert_eq!(active_line(&l, 4.9), Some(0));
        assert_eq!(active_line(&l, 8.9), Some(1));
        assert_eq!(active_line(&l, 200.0), Some(1));
    }

    #[test]
    fn empty_and_invalid_positions() {
        assert_eq!(active_line(&[], 10.0), None);
        let l = lines(&[0, 1_000]);
        assert_eq!(active_line(&l, f64::NAN), None);
        assert_eq!(active_line(&l, -1.0), None);
        assert_eq!(active_line(&l, 0.0), Some(0));
    }

    #[test]
    fn manual_scroll_pauses_auto_scroll() {
        assert!(should_autoscroll(1_000, None));
        assert!(!should_autoscroll(5_000, Some(2_000)));
        assert!(should_autoscroll(6_000, Some(2_000)));
        assert!(!should_autoscroll(0, Some(10_000)));
    }

    #[test]
    fn opacity_falls_off_with_distance() {
        assert_eq!(line_opacity(Some(5), 5), 1.0);
        assert_eq!(line_opacity(Some(5), 4), 0.5);
        assert_eq!(line_opacity(Some(5), 7), 0.28);
        assert_eq!(line_opacity(Some(5), 40), 0.2);
        assert_eq!(line_opacity(None, 0), 0.5);
        assert_eq!(line_opacity(None, 9), 0.2);
    }
}
