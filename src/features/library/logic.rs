//! Pure helpers for the library screens: cover mosaics, playlist-name validation, day grouping
//! of the history and the sidebar "recent playlists" pick. No UI, no I/O.

use crate::model::{Playlist, Track};
use crate::store::library::{HistoryEntry, PLAYLIST_NAME_MAX};

const SECS_PER_DAY: i64 = 86_400;

// ---------------------------------------------------------------- cover mosaic

/// What a playlist cover shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mosaic {
    /// No usable artwork: the UI draws an icon.
    Empty,
    /// One distinct cover: shown full size.
    Single(String),
    /// Four cells (2 x 2). Fewer than four distinct covers are repeated so the grid has no holes.
    Grid([String; 4]),
}

/// Pick the artwork for a playlist from its first tracks (distinct covers only, in order).
/// A playlist's artwork: the picture the user chose, else the mosaic of its tracks.
pub fn playlist_art(p: &crate::model::Playlist) -> Mosaic {
    match p.cover.as_deref().filter(|c| !c.is_empty()) {
        Some(c) => Mosaic::Single(c.to_owned()),
        None => mosaic(&p.tracks),
    }
}

pub fn mosaic(tracks: &[Track]) -> Mosaic {
    let mut urls: Vec<String> = Vec::with_capacity(4);
    for url in tracks
        .iter()
        .filter_map(|t| t.cover.as_ref())
        .map(|c| c.url.trim())
        .filter(|u| !u.is_empty())
    {
        if !urls.iter().any(|u| u == url) {
            urls.push(url.to_owned());
            if urls.len() == 4 {
                break;
            }
        }
    }
    match urls.as_slice() {
        [] => Mosaic::Empty,
        [one] => Mosaic::Single(one.clone()),
        [a, b] => Mosaic::Grid([a.clone(), b.clone(), b.clone(), a.clone()]),
        [a, b, c] => Mosaic::Grid([a.clone(), b.clone(), c.clone(), a.clone()]),
        [a, b, c, d, ..] => Mosaic::Grid([a.clone(), b.clone(), c.clone(), d.clone()]),
    }
}

/// First artwork URL of a mosaic, used as the blurred hero backdrop.
pub fn backdrop(tracks: &[Track]) -> Option<String> {
    match mosaic(tracks) {
        Mosaic::Empty => None,
        Mosaic::Single(u) => Some(u),
        Mosaic::Grid([a, ..]) => Some(a),
    }
}

// ----------------------------------------------------------------- name rules

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Duplicate,
}

impl NameError {
    /// i18n key with the plain-words explanation.
    pub fn key(self) -> &'static str {
        match self {
            NameError::Empty => "library.name.empty",
            NameError::TooLong => "library.name.too_long",
            NameError::Duplicate => "library.name.duplicate",
        }
    }
}

fn fold_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Check a playlist name against the names of the user's *other* playlists. Returns the cleaned
/// name (trimmed, inner whitespace collapsed) or what is wrong with it. Names that differ only in
/// case or spacing count as duplicates.
pub fn validate_name(raw: &str, others: &[&str]) -> Result<String, NameError> {
    let clean = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() {
        return Err(NameError::Empty);
    }
    if clean.chars().count() > PLAYLIST_NAME_MAX {
        return Err(NameError::TooLong);
    }
    let folded = fold_name(&clean);
    if others.iter().any(|o| fold_name(o) == folded) {
        return Err(NameError::Duplicate);
    }
    Ok(clean)
}

// ---------------------------------------------------------------- history days

/// Consecutive history entries played on the same local day.
#[derive(Clone, Debug, PartialEq)]
pub struct DayGroup {
    /// Days since 1970-01-01 in local time.
    pub day: i64,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayLabel {
    Today,
    Yesterday,
    Date { year: i32, month: u32, day: u32 },
}

/// Local day number of a Unix-millisecond timestamp; `east_min` is the UTC offset in minutes
/// (positive east of Greenwich).
pub fn local_day(ms: u64, east_min: i32) -> i64 {
    let secs = i64::try_from(ms / 1000).unwrap_or(i64::MAX / 2) + i64::from(east_min) * 60;
    secs.div_euclid(SECS_PER_DAY)
}

/// Group a newest-first history into days (order is kept: newest day first).
pub fn group_by_day(history: &[HistoryEntry], east_min: i32) -> Vec<DayGroup> {
    history.iter().fold(Vec::new(), |groups: Vec<DayGroup>, h| {
        let day = local_day(h.played_at_ms, east_min);
        match groups.split_last() {
            Some((last, rest)) if last.day == day => rest
                .iter()
                .cloned()
                .chain(std::iter::once(DayGroup {
                    day,
                    tracks: [last.tracks.clone(), vec![h.track.clone()]].concat(),
                }))
                .collect(),
            _ => groups
                .into_iter()
                .chain(std::iter::once(DayGroup {
                    day,
                    tracks: vec![h.track.clone()],
                }))
                .collect(),
        }
    })
}

/// "14:05" (24-hour, local time) of a Unix-millisecond timestamp.
pub fn time_label(ms: u64, east_min: i32) -> String {
    let secs = i64::try_from(ms / 1000).unwrap_or(0) + i64::from(east_min) * 60;
    let of_day = secs.rem_euclid(SECS_PER_DAY);
    format!("{:02}:{:02}", of_day / 3600, of_day % 3600 / 60)
}

/// Civil date of a day number (proleptic Gregorian calendar).
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

/// "Today", "Yesterday" or a calendar date for `day`, seen from `today`.
pub fn day_label(day: i64, today: i64) -> DayLabel {
    if day >= today {
        DayLabel::Today
    } else if day == today - 1 {
        DayLabel::Yesterday
    } else {
        let (year, month, day) = civil_from_days(day);
        DayLabel::Date { year, month, day }
    }
}

// ------------------------------------------------------------------ summaries

/// Total length of a track list in milliseconds.
pub fn total_ms(tracks: &[Track]) -> u64 {
    tracks
        .iter()
        .fold(0u64, |acc, t| acc.saturating_add(t.duration_ms))
}

/// "12 şarkı · 45 dk" for a playlist.
pub fn playlist_summary(p: &Playlist) -> String {
    crate::features::tracks::format::songs_summary(p.tracks.len(), total_ms(&p.tracks))
}

/// The `n` most recently updated playlists (newest first; ties favour the later created).
pub fn recent_playlists(playlists: &[Playlist], n: usize) -> Vec<Playlist> {
    let mut sorted: Vec<Playlist> = playlists.to_vec();
    sorted.sort_by(|a, b| {
        b.updated_ms
            .cmp(&a.updated_ms)
            .then(b.created_ms.cmp(&a.created_ms))
    });
    sorted.into_iter().take(n).collect()
}

/// The library tabs; unknown values fall back to the playlists tab.
pub const TABS: [&str; 5] = ["playlists", "liked", "downloads", "history", "local"];

pub fn normalize_tab(tab: &str) -> &'static str {
    TABS.iter().copied().find(|t| *t == tab).unwrap_or(TABS[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cover;

    #[test]
    fn time_label_is_local_24_hour() {
        assert_eq!(time_label(0, 0), "00:00");
        assert_eq!(time_label(14 * 3_600_000 + 5 * 60_000, 0), "14:05");
        assert_eq!(time_label(23 * 3_600_000, 180), "02:00");
    }

    #[test]
    fn backdrop_is_first_cover() {
        assert_eq!(backdrop(&[]), None);
        assert_eq!(
            backdrop(&[track("a", None, 1), track("b", Some("u"), 1)]),
            Some("u".to_string())
        );
    }

    fn track(id: &str, cover: Option<&str>, ms: u64) -> Track {
        Track {
            id: id.into(),
            duration_ms: ms,
            cover: cover.map(|u| Cover {
                url: u.into(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn played(id: &str, at: u64) -> HistoryEntry {
        HistoryEntry {
            track: track(id, None, 0),
            played_at_ms: at,
            ..Default::default()
        }
    }

    fn grid(m: Mosaic) -> [String; 4] {
        match m {
            Mosaic::Grid(g) => g,
            other => panic!("expected a grid, got {other:?}"),
        }
    }

    #[test]
    fn mosaic_is_empty_without_artwork() {
        assert_eq!(mosaic(&[]), Mosaic::Empty);
        let t = [track("a", None, 0), track("b", Some(" "), 0)];
        assert_eq!(mosaic(&t), Mosaic::Empty);
    }

    #[test]
    fn mosaic_uses_one_cover_for_identical_art() {
        let t = [track("a", Some("x"), 0), track("b", Some("x"), 0)];
        assert_eq!(mosaic(&t), Mosaic::Single("x".into()));
    }

    #[test]
    fn mosaic_fills_four_cells_from_few_covers() {
        let two = [track("a", Some("x"), 0), track("b", Some("y"), 0)];
        assert_eq!(grid(mosaic(&two)), ["x", "y", "y", "x"].map(String::from));
        let three = [
            track("a", Some("x"), 0),
            track("n", None, 0),
            track("b", Some("y"), 0),
            track("c", Some("z"), 0),
        ];
        assert_eq!(grid(mosaic(&three)), ["x", "y", "z", "x"].map(String::from));
    }

    #[test]
    fn mosaic_takes_the_first_four_distinct() {
        let t: Vec<Track> = ["a", "b", "a", "c", "d", "e"]
            .iter()
            .map(|u| track(u, Some(u), 0))
            .collect();
        assert_eq!(grid(mosaic(&t)), ["a", "b", "c", "d"].map(String::from));
    }

    #[test]
    fn names_are_cleaned_and_checked() {
        assert_eq!(
            validate_name("  Yaz   hitleri ", &[]),
            Ok("Yaz hitleri".into())
        );
        assert_eq!(validate_name("   ", &[]), Err(NameError::Empty));
        let long = "a".repeat(PLAYLIST_NAME_MAX + 1);
        assert_eq!(validate_name(&long, &[]), Err(NameError::TooLong));
        let max = "a".repeat(PLAYLIST_NAME_MAX);
        assert_eq!(validate_name(&max, &[]), Ok(max.clone()));
    }

    #[test]
    fn duplicates_ignore_case_and_spacing() {
        let others = ["Yaz Hitleri", "Rock"];
        assert_eq!(
            validate_name("yaz  hitleri", &others),
            Err(NameError::Duplicate)
        );
        assert_eq!(validate_name(" ROCK ", &others), Err(NameError::Duplicate));
        assert_eq!(validate_name("Rock 2", &others), Ok("Rock 2".into()));
    }

    #[test]
    fn every_error_has_a_key() {
        for e in [NameError::Empty, NameError::TooLong, NameError::Duplicate] {
            assert!(e.key().starts_with("library.name."));
        }
    }

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
    }

    #[test]
    fn local_day_respects_the_offset() {
        let late = 86_400_000 - 1_000; // 23:59:59 UTC on day 0
        assert_eq!(local_day(late, 0), 0);
        assert_eq!(local_day(late, 180), 1); // UTC+3 is already tomorrow
        assert_eq!(local_day(1_000, -300), -1);
    }

    #[test]
    fn history_groups_by_day_in_order() {
        let day = 86_400_000u64;
        let h = [
            played("a", 5 * day + 3_000),
            played("b", 5 * day + 1_000),
            played("c", 4 * day + 9_000),
            played("d", 2 * day),
        ];
        let g = group_by_day(&h, 0);
        assert_eq!(g.iter().map(|x| x.day).collect::<Vec<_>>(), [5, 4, 2]);
        let first: Vec<String> = g[0].tracks.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(first, ["a", "b"]);
        assert!(group_by_day(&[], 0).is_empty());
    }

    #[test]
    fn day_labels() {
        assert_eq!(day_label(10, 10), DayLabel::Today);
        assert_eq!(day_label(11, 10), DayLabel::Today);
        assert_eq!(day_label(9, 10), DayLabel::Yesterday);
        assert_eq!(
            day_label(0, 10),
            DayLabel::Date {
                year: 1970,
                month: 1,
                day: 1
            }
        );
    }

    #[test]
    fn playlist_totals() {
        let tracks = [
            track("a", None, 60_000),
            track("b", None, 150_000),
            track("c", None, u64::MAX),
        ];
        assert_eq!(total_ms(&tracks[..2]), 210_000);
        assert_eq!(total_ms(&tracks), u64::MAX);
        let p = Playlist {
            tracks: tracks[..2].to_vec(),
            ..Default::default()
        };
        let s = playlist_summary(&p);
        assert!(s.contains('2') && s.contains('3'), "{s}");
    }

    #[test]
    fn recent_playlists_are_newest_first_and_capped() {
        let mk = |id: &str, up: u64, cr: u64| Playlist {
            id: id.into(),
            updated_ms: up,
            created_ms: cr,
            ..Default::default()
        };
        let all = [mk("a", 5, 1), mk("b", 9, 2), mk("c", 5, 3), mk("d", 1, 4)];
        let ids: Vec<String> = recent_playlists(&all, 3)
            .iter()
            .map(|p| p.id.to_string())
            .collect();
        assert_eq!(ids, ["b", "c", "a"]);
        assert!(recent_playlists(&[], 6).is_empty());
    }

    #[test]
    fn unknown_tabs_fall_back() {
        assert_eq!(normalize_tab("history"), "history");
        assert_eq!(normalize_tab("nope"), "playlists");
    }
}
