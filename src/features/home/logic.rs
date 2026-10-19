//! Pure Home logic: which songs seed the mixes, what the starter suggestions search for.

use std::collections::HashSet;

use super::cache::Mix;
use crate::model::{Cover, Track};
use crate::store::library::HistoryEntry;

pub const MAX_SEEDS: usize = 3;
pub const MIX_MAX_TRACKS: usize = 25;
pub const RECENT_MAX: usize = 12;

/// A song the mixes are built around.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub id: String,
    pub artist: String,
    pub cover: Option<Cover>,
}

fn artist_key(t: &Track) -> Option<String> {
    let a = t.artists.first()?;
    let key = if a.id.is_empty() {
        a.name.to_lowercase()
    } else {
        a.id.to_string()
    };
    (!key.is_empty()).then_some(key)
}

/// Up to `max` seeds from what the user played most recently, then liked songs: one song per
/// artist, never local files (they have no catalog neighbours). Empty when there is no history.
pub fn pick_seeds(history: &[HistoryEntry], liked: &[Track], max: usize) -> Vec<Seed> {
    let mut seen = HashSet::new();
    history
        .iter()
        .map(|h| &h.track)
        .chain(liked.iter())
        .filter(|t| !t.is_local())
        .filter_map(|t| {
            let key = artist_key(t)?;
            seen.insert(key).then(|| Seed {
                id: t.id.to_string(),
                artist: t.artists_label(),
                cover: t.cover.clone(),
            })
        })
        .take(max)
        .collect()
}

/// A mix from radio results: the seed itself and repeats are dropped, length capped.
/// `None` when nothing is left.
pub fn build_mix(seed: &Seed, radio: Vec<Track>) -> Option<Mix> {
    let mut seen = HashSet::new();
    let tracks: Vec<Track> = radio
        .into_iter()
        .filter(|t| t.id != seed.id && seen.insert(t.id.clone()))
        .take(MIX_MAX_TRACKS)
        .collect();
    (!tracks.is_empty()).then(|| Mix {
        seed_id: seed.id.clone(),
        artist: seed.artist.clone(),
        cover: seed.cover.clone(),
        tracks,
    })
}

/// The newest `n` played tracks.
pub fn recent_tracks(history: &[HistoryEntry], n: usize) -> Vec<Track> {
    history.iter().take(n).map(|h| h.track.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ArtistRef;

    fn track(id: &str, artist: &str) -> Track {
        Track {
            id: id.into(),
            artists: if artist.is_empty() {
                vec![]
            } else {
                vec![ArtistRef {
                    id: format!("UC{artist}").into(),
                    name: artist.into(),
                }]
            },
            ..Default::default()
        }
    }

    fn entry(t: Track) -> HistoryEntry {
        HistoryEntry {
            track: t,
            played_at_ms: 0,
            ..Default::default()
        }
    }

    #[test]
    fn seeds_are_one_song_per_artist_newest_first() {
        let history = vec![
            entry(track("a1", "A")),
            entry(track("a2", "A")),
            entry(track("b1", "B")),
        ];
        let liked = vec![track("c1", "C"), track("b2", "B")];
        let ids: Vec<_> = pick_seeds(&history, &liked, 3)
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, ["a1", "b1", "c1"]);
    }

    #[test]
    fn seeds_skip_local_files_and_unknown_artists() {
        let history = vec![entry(track("local-1", "A")), entry(track("x", ""))];
        assert!(pick_seeds(&history, &[], 3).is_empty());
    }

    #[test]
    fn seeds_respect_the_cap() {
        let liked: Vec<_> = (0..10)
            .map(|i| track(&format!("t{i}"), &format!("Art{i}")))
            .collect();
        assert_eq!(pick_seeds(&[], &liked, MAX_SEEDS).len(), MAX_SEEDS);
    }

    #[test]
    fn mix_drops_seed_and_duplicates_and_caps_length() {
        let seed = Seed {
            id: "s".into(),
            artist: "A".into(),
            cover: None,
        };
        let mut radio = vec![track("s", "A"), track("x", "B"), track("x", "B")];
        radio.extend((0..40).map(|i| track(&format!("n{i}"), "C")));
        let mix = build_mix(&seed, radio).unwrap();
        assert_eq!(mix.tracks.len(), MIX_MAX_TRACKS);
        assert_eq!(mix.tracks[0].id, "x");
        assert!(mix.tracks.iter().all(|t| t.id != "s"));
    }

    #[test]
    fn empty_radio_gives_no_mix() {
        let seed = Seed {
            id: "s".into(),
            artist: "A".into(),
            cover: None,
        };
        assert!(build_mix(&seed, vec![track("s", "A")]).is_none());
    }

    #[test]
    fn recent_is_capped_and_ordered() {
        let history: Vec<_> = (0..20)
            .map(|i| entry(track(&format!("t{i}"), "A")))
            .collect();
        let r = recent_tracks(&history, RECENT_MAX);
        assert_eq!(r.len(), RECENT_MAX);
        assert_eq!(r[0].id, "t0");
    }
}
