//! Pure numbers behind the listening stats page (from the play history).

use std::collections::HashMap;

use crate::model::Track;
use crate::store::library::HistoryEntry;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub plays: u32,
    pub tracks: usize,
    pub artists: usize,
    pub skips: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistCount {
    pub name: String,
    /// Catalog id, empty when unknown.
    pub id: String,
    pub plays: u32,
}

fn lead_name(t: &Track) -> Option<&str> {
    t.artists
        .first()
        .map(|a| a.name.trim())
        .filter(|n| !n.is_empty())
}

pub fn totals(history: &[HistoryEntry]) -> Totals {
    Totals {
        plays: history.iter().map(HistoryEntry::play_count).sum(),
        tracks: history.len(),
        artists: top_artists(history, usize::MAX).len(),
        skips: history.iter().map(|h| h.skips).sum(),
    }
}

/// Most played tracks with their counts; ties keep the most recent first.
pub fn top_tracks(history: &[HistoryEntry], n: usize) -> Vec<(Track, u32)> {
    let mut ranked: Vec<&HistoryEntry> = history.iter().collect();
    ranked.sort_by_key(|h| std::cmp::Reverse(h.play_count()));
    ranked
        .into_iter()
        .take(n)
        .map(|h| (h.track.clone(), h.play_count()))
        .collect()
}

/// Lead artists by summed plays, merged by lower-cased name; ties keep first-seen (recent) order.
pub fn top_artists(history: &[HistoryEntry], n: usize) -> Vec<ArtistCount> {
    let mut order: Vec<ArtistCount> = vec![];
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for h in history {
        let Some(name) = lead_name(&h.track) else {
            continue;
        };
        let at = *by_name.entry(name.to_lowercase()).or_insert_with(|| {
            order.push(ArtistCount {
                name: name.to_owned(),
                id: String::new(),
                plays: 0,
            });
            order.len() - 1
        });
        let slot = &mut order[at];
        slot.plays += h.play_count();
        if slot.id.is_empty() {
            slot.id = h.track.artists[0].id.to_string();
        }
    }
    order.sort_by_key(|a| std::cmp::Reverse(a.plays));
    order.truncate(n);
    order
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("logic_tests.rs");
}
