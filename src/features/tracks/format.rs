//! Text helpers for counts and durations ("12 şarkı", "1 sa 5 dk").

use crate::i18n::{t_args, t_count};

/// Whole hours and minutes (rounded down) of a duration; under a minute counts as one minute so
/// a short list never reads "0 dk".
pub fn hours_minutes(ms: u64) -> (u64, u64) {
    let minutes = (ms / 60_000).max(1);
    (minutes / 60, minutes % 60)
}

/// "45 dk" or "1 sa 5 dk" in the current language. Empty when the duration is unknown (0).
pub fn duration_text(ms: u64) -> String {
    if ms == 0 {
        return String::new();
    }
    let (h, m) = hours_minutes(ms);
    if h == 0 {
        t_args("common.minutes", &[("minutes", &m.to_string())])
    } else {
        t_args(
            "common.hours_minutes",
            &[("hours", &h.to_string()), ("minutes", &m.to_string())],
        )
    }
}

/// "12 şarkı · 45 dk" (the duration part is left out when unknown).
pub fn songs_summary(count: usize, total_ms: u64) -> String {
    let songs = t_count("common.songs", count);
    let duration = duration_text(total_ms);
    if duration.is_empty() {
        songs
    } else {
        format!("{songs} · {duration}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_hours_and_minutes() {
        assert_eq!(hours_minutes(45 * 60_000), (0, 45));
        assert_eq!(hours_minutes(65 * 60_000 + 30_000), (1, 5));
        assert_eq!(hours_minutes(7_200_000), (2, 0));
    }

    #[test]
    fn very_short_lists_count_as_one_minute() {
        assert_eq!(hours_minutes(10_000), (0, 1));
    }

    #[test]
    fn unknown_duration_has_no_text() {
        assert_eq!(duration_text(0), "");
    }
}
