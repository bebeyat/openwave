//! Pure logic of the search page: request ordering, view phases, tabs, suggestions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::backend::api::SearchKind;
use crate::i18n::Lang;
use crate::model::SearchResults;

/// Wait this long after the last keystroke before asking the catalog.
pub const DEBOUNCE_MS: u64 = 250;
/// A term is added to "Son aramalar" only after its results stayed on screen this long.
pub const REMEMBER_AFTER_MS: u64 = 1500;

/// Hands out increasing tickets; only the newest ticket may publish a result, so a slow old
/// request can never overwrite a newer one (out-of-order responses are ignored).
#[derive(Clone, Debug, Default)]
pub struct RequestGuard(Arc<AtomicU64>);

impl RequestGuard {
    /// Start a new request; every older ticket becomes stale.
    pub fn advance(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn is_current(&self, ticket: u64) -> bool {
        self.0.load(Ordering::SeqCst) == ticket
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Songs,
    Artists,
    Albums,
    Playlists,
    SoundCloud,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Songs,
        Tab::Artists,
        Tab::Albums,
        Tab::Playlists,
        Tab::SoundCloud,
    ];

    pub fn value(self) -> &'static str {
        match self {
            Tab::Songs => "songs",
            Tab::Artists => "artists",
            Tab::Albums => "albums",
            Tab::Playlists => "playlists",
            Tab::SoundCloud => "soundcloud",
        }
    }

    pub fn parse(value: &str) -> Tab {
        Tab::ALL
            .into_iter()
            .find(|t| t.value() == value)
            .unwrap_or_default()
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Tab::Songs => "search.tab.songs",
            Tab::Artists => "search.tab.artists",
            Tab::Albums => "search.tab.albums",
            Tab::Playlists => "search.tab.playlists",
            Tab::SoundCloud => "search.tab.soundcloud",
        }
    }

    pub fn kind(self) -> SearchKind {
        match self {
            Tab::Songs => SearchKind::Track,
            Tab::Artists => SearchKind::Artist,
            Tab::Albums => SearchKind::Album,
            Tab::Playlists => SearchKind::Playlist,
            Tab::SoundCloud => SearchKind::SoundCloud,
        }
    }

    /// How many items `results` holds for this tab.
    pub fn count(self, results: &SearchResults) -> usize {
        match self {
            Tab::Songs | Tab::SoundCloud => results.tracks.len(),
            Tab::Artists => results.artists.len(),
            Tab::Albums => results.albums.len(),
            Tab::Playlists => results.playlists.len(),
        }
    }
}

/// What the results area shows. Only `Done` with zero items may say "Sonuç bulunamadı".
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Loading,
    Done(Arc<SearchResults>),
    /// Translated, user-facing message.
    Failed(String),
}

impl Phase {
    /// True only for a COMPLETED request with nothing in it for `tab`.
    pub fn shows_no_results(&self, tab: Tab) -> bool {
        matches!(self, Phase::Done(r) if tab.count(r) == 0)
    }
}

/// Trim and collapse inner whitespace; the empty string means "nothing to search".
pub fn normalize_query(q: &str) -> String {
    q.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fixed example searches for the empty state (no personalization is implied).
pub fn suggested_searches(lang: Lang) -> [&'static str; 6] {
    match lang {
        Lang::Tr => [
            "Türkçe pop",
            "Rock klasikleri",
            "Çalışırken dinlenecek müzik",
            "Caz",
            "Film müzikleri",
            "90'lar hitleri",
        ],
        _ => [
            "Pop hits",
            "Classic rock",
            "Music to study to",
            "Jazz",
            "Movie soundtracks",
            "90s hits",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArtistItem, Track};

    #[test]
    fn only_the_newest_ticket_is_current() {
        let g = RequestGuard::default();
        let first = g.advance();
        assert!(g.is_current(first));
        let second = g.advance();
        assert!(!g.is_current(first));
        assert!(g.is_current(second));
    }

    #[test]
    fn clones_share_one_counter() {
        let g = RequestGuard::default();
        let other = g.clone();
        let t = g.advance();
        other.advance();
        assert!(!g.is_current(t));
    }

    #[test]
    fn out_of_order_results_are_dropped() {
        // Simulate responses arriving in the order 2, 1: only ticket 2 may publish.
        let g = RequestGuard::default();
        let (t1, t2) = (g.advance(), g.advance());
        let mut published = vec![];
        for t in [t2, t1] {
            if g.is_current(t) {
                published.push(t);
            }
        }
        assert_eq!(published, vec![t2]);
    }

    #[test]
    fn no_results_is_never_shown_while_loading_or_idle_or_failed() {
        for tab in Tab::ALL {
            assert!(!Phase::Idle.shows_no_results(tab));
            assert!(!Phase::Loading.shows_no_results(tab));
            assert!(!Phase::Failed("x".into()).shows_no_results(tab));
            assert!(Phase::Done(Arc::new(SearchResults::default())).shows_no_results(tab));
        }
    }

    #[test]
    fn no_results_depends_on_the_tab() {
        let r = SearchResults {
            tracks: vec![Track::default()],
            ..Default::default()
        };
        let done = Phase::Done(Arc::new(r));
        assert!(!done.shows_no_results(Tab::Songs));
        assert!(done.shows_no_results(Tab::Artists));
        let r = SearchResults {
            artists: vec![ArtistItem::default()],
            ..Default::default()
        };
        assert!(!Phase::Done(Arc::new(r)).shows_no_results(Tab::Artists));
    }

    #[test]
    fn tabs_round_trip_and_default_to_songs() {
        for tab in Tab::ALL {
            assert_eq!(Tab::parse(tab.value()), tab);
        }
        assert_eq!(Tab::parse("nope"), Tab::Songs);
    }

    #[test]
    fn queries_are_normalized() {
        assert_eq!(normalize_query("  daft   punk \n"), "daft punk");
        assert_eq!(normalize_query("   "), "");
    }

    #[test]
    fn suggestions_exist_in_both_languages() {
        for lang in Lang::ALL {
            let s = suggested_searches(lang);
            assert!(s.iter().all(|x| !x.trim().is_empty()));
        }
    }
}
