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

/// One press of the lyrics shift buttons.
pub const OFFSET_STEP_MS: i32 = 500;

/// The playback position as the lyrics see it: a positive shift makes every line come later.
pub fn shifted(position_secs: f64, offset_ms: i32) -> f64 {
    position_secs - f64::from(offset_ms) / 1000.0
}

/// "+0.5s", "-1.0s", "0.0s".
pub fn offset_label(offset_ms: i32) -> String {
    let sign = if offset_ms > 0 { "+" } else { "" };
    format!("{sign}{:.1}s", f64::from(offset_ms) / 1000.0)
}

/// Whether `text` holds right-to-left script (Hebrew, Arabic, Syriac, Thaana, N'Ko and their
/// presentation forms), so its line is laid out with `dir="rtl"`.
pub fn is_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
            0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
    })
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

/// Estimated singing pace for lines without word stamps, and the bounds of one line's fill.
const MS_PER_CHAR: u64 = 110;
const MIN_FILL_MS: u64 = 1_200;

/// One word of a lyric line with the time span its karaoke fill runs over. `text` keeps the
/// trailing space so the spans join back into the line.
#[derive(Clone, Debug, PartialEq)]
pub struct WordSpan {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Word fill timings for line `i`: the enhanced-LRC word stamps when the line has them, otherwise
/// the words spread over the line by length. An estimated line ends at the next line or after a
/// singing-pace guess, whichever comes first, so an instrumental gap does not stretch the fill.
pub fn word_spans(lines: &[LyricLine], i: usize) -> Vec<WordSpan> {
    let Some(line) = lines.get(i) else {
        return vec![];
    };
    if !line.words.is_empty() {
        return line
            .words
            .iter()
            .map(|w| WordSpan {
                text: w.text.clone(),
                start_ms: w.start_ms,
                end_ms: w.end_ms.max(w.start_ms + 1),
            })
            .collect();
    }
    let words: Vec<&str> = line.text.trim().split_inclusive(' ').collect();
    let weight = |w: &str| w.trim().chars().count().max(1) as u64;
    let total: u64 = words.iter().map(|w| weight(w)).sum();
    let guess = (total * MS_PER_CHAR).max(MIN_FILL_MS);
    let next = lines.get(i + 1).map_or(u64::MAX, |l| l.start_ms);
    let end = (line.start_ms + guess).min(next).max(line.start_ms + 1);
    let span = end - line.start_ms;
    let mut done = 0;
    words
        .into_iter()
        .map(|w| {
            let start_ms = line.start_ms + span * done / total;
            done += weight(w);
            WordSpan {
                text: w.to_owned(),
                start_ms,
                end_ms: line.start_ms + span * done / total,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LyricWord;

    fn line(start_ms: u64, text: &str) -> LyricLine {
        LyricLine {
            start_ms,
            text: text.into(),
            words: vec![],
        }
    }

    #[test]
    fn words_share_the_line_by_length_and_join_back() {
        let l = [line(1_000, "ab abcd"), line(2_200, "next")];
        let w = word_spans(&l, 0);
        assert_eq!(
            w.iter().map(|w| w.text.as_str()).collect::<String>(),
            "ab abcd"
        );
        // 6 letters over 1.2 s (the next line comes before the pace guess runs out).
        assert_eq!((w[0].start_ms, w[0].end_ms), (1_000, 1_400));
        assert_eq!((w[1].start_ms, w[1].end_ms), (1_400, 2_200));
    }

    #[test]
    fn a_long_gap_does_not_stretch_the_fill() {
        let l = [line(0, "hey you"), line(60_000, "after the solo")];
        let w = word_spans(&l, 0);
        assert_eq!(w.last().unwrap().end_ms, MIN_FILL_MS);
        // The last line has no next line: it uses the pace guess.
        let last = word_spans(&l, 1);
        assert_eq!(last.last().unwrap().end_ms, 60_000 + 12 * MS_PER_CHAR);
    }

    #[test]
    fn stamped_words_are_used_as_they_are() {
        let mut l = line(1_000, "Hello");
        l.words = vec![
            LyricWord {
                start_ms: 1_000,
                end_ms: 1_500,
                text: "Hel".into(),
            },
            LyricWord {
                start_ms: 1_500,
                end_ms: 1_500,
                text: "lo".into(),
            },
        ];
        let w = word_spans(&[l], 0);
        assert_eq!((w[0].start_ms, w[0].end_ms), (1_000, 1_500));
        assert_eq!((w[1].start_ms, w[1].end_ms), (1_500, 1_501));
    }

    #[test]
    fn empty_and_missing_lines() {
        assert!(word_spans(&[], 0).is_empty());
        let w = word_spans(&[line(500, "  ")], 0);
        assert!(w.iter().all(|w| w.end_ms > w.start_ms));
    }

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

// Local-only tests (see build.rs).
#[cfg(all(test, private_tests))]
mod parity_tests {
    include!("lyric_logic_parity_tests.rs");
}
