//! On-disk caches for the taste rows, so Home paints them at once (and offline):
//!
//! * `home_genres.json`: the genre index, sample tracks of every catalog genre. Rebuilt weekly
//!   in the background (about 80 small catalog requests, a few at a time).
//! * `home_recs.json`: the last "Senin için önerilenler" list and the seeds it came from.

use std::path::Path;

use futures_util::{stream, StreamExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::taste::GenreSample;
use crate::backend::api;
use crate::model::{CollectionKind, Track};
use crate::store::persist::write_atomic;

pub const GENRES_FILE: &str = "home_genres.json";
pub const RECS_FILE: &str = "home_recs.json";
pub const GENRES_MAX_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1000;
pub const RECS_MAX_AGE_MS: u64 = 6 * 60 * 60 * 1000;
/// A list younger than this is kept even when the seeds changed (one play reorders them).
pub const RECS_MIN_AGE_MS: u64 = 20 * 60 * 1000;
/// Genres taken from the catalog list (it has about 26).
const GENRES_MAX: usize = 32;
/// Playlists read per genre, and tracks kept per genre.
const PLAYLISTS_PER_GENRE: usize = 2;
const TRACKS_PER_GENRE: usize = 100;
/// Genres fetched at the same time.
const PARALLEL: usize = 3;
/// Catalog "genres" that group by era or audience rather than style; their playlists hold every
/// style, so they would match almost anyone.
const NOT_GENRES: [&str; 2] = ["decades", "family"];

fn is_style(name: &str) -> bool {
    let n = name.to_lowercase();
    !NOT_GENRES.iter().any(|skip| n == *skip)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenreIndex {
    pub saved_ms: u64,
    pub genres: Vec<GenreSample>,
}

impl GenreIndex {
    pub fn is_fresh(&self, now_ms: u64) -> bool {
        !self.genres.is_empty() && now_ms.saturating_sub(self.saved_ms) < GENRES_MAX_AGE_MS
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecsCache {
    pub saved_ms: u64,
    pub seed_ids: Vec<String>,
    pub tracks: Vec<Track>,
}

impl RecsCache {
    /// Fresh when younger than [`RECS_MIN_AGE_MS`], or built from the same seeds (in any order)
    /// less than [`RECS_MAX_AGE_MS`] ago.
    pub fn is_fresh(&self, seed_ids: &[String], now_ms: u64) -> bool {
        let age = now_ms.saturating_sub(self.saved_ms);
        let mut had = self.seed_ids.clone();
        let mut want = seed_ids.to_vec();
        had.sort();
        want.sort();
        !self.tracks.is_empty() && (age < RECS_MIN_AGE_MS || (had == want && age < RECS_MAX_AGE_MS))
    }
}

/// Missing or unreadable files give the default (the caches are only a convenience).
pub fn load<T: DeserializeOwned + Default>(dir: &Path, file: &str) -> T {
    std::fs::read(dir.join(file))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save<T: Serialize>(dir: &Path, file: &str, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        // Best effort: a failed write only means the next start rebuilds.
        let _ = write_atomic(dir, file, &bytes);
    }
}

/// Sample tracks of one genre from its first playlists; `None` when nothing could be read.
async fn sample_genre(name: String, key: String) -> Option<GenreSample> {
    let lists = api::genre_playlists(&key).await.ok()?;
    let mut tracks: Vec<Track> = vec![];
    for list in lists.iter().take(PLAYLISTS_PER_GENRE) {
        if let Ok(c) = api::collection(CollectionKind::Playlist, &list.key).await {
            tracks.extend(c.tracks);
        }
    }
    tracks.truncate(TRACKS_PER_GENRE);
    (!tracks.is_empty()).then_some(GenreSample { name, key, tracks })
}

/// Build the genre index from the catalog. `None` when the catalog is unreachable or empty.
pub async fn build_genres(now_ms: u64) -> Option<GenreIndex> {
    let list = api::genres().await.ok()?;
    let styles = list
        .into_iter()
        .filter(|g| is_style(&g.name))
        .take(GENRES_MAX);
    let genres: Vec<GenreSample> = stream::iter(styles)
        .map(|g| sample_genre(g.name, g.key))
        .buffered(PARALLEL)
        .filter_map(|g| async move { g })
        .collect()
        .await;
    (!genres.is_empty()).then_some(GenreIndex {
        saved_ms: now_ms,
        genres,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(saved_ms: u64, n: usize) -> GenreIndex {
        GenreIndex {
            saved_ms,
            genres: vec![GenreSample::default(); n],
        }
    }

    #[test]
    fn era_and_audience_groups_are_not_styles() {
        assert!(!is_style("Decades") && !is_style("family"));
        assert!(is_style("Turkish pop") && is_style("Family pop"));
    }

    #[test]
    fn genre_index_is_fresh_for_a_week_when_not_empty() {
        assert!(index(1_000, 1).is_fresh(1_000 + GENRES_MAX_AGE_MS - 1));
        assert!(!index(1_000, 1).is_fresh(1_000 + GENRES_MAX_AGE_MS));
        assert!(!index(1_000, 0).is_fresh(1_001));
    }

    #[test]
    fn recs_stay_while_young_or_built_from_the_same_seed_set() {
        let c = RecsCache {
            saved_ms: 10,
            seed_ids: vec!["a".into(), "b".into()],
            tracks: vec![Track::default()],
        };
        let reordered = vec!["b".to_string(), "a".to_string()];
        let other = vec!["c".to_string()];
        assert!(c.is_fresh(&reordered, 10 + RECS_MAX_AGE_MS - 1));
        assert!(!c.is_fresh(&reordered, 10 + RECS_MAX_AGE_MS));
        assert!(c.is_fresh(&other, 10 + RECS_MIN_AGE_MS - 1));
        assert!(!c.is_fresh(&other, 10 + RECS_MIN_AGE_MS));
        assert!(!RecsCache::default().is_fresh(&[], 0));
    }

    #[test]
    fn caches_round_trip_and_survive_garbage() {
        let dir = std::env::temp_dir().join(format!("ow-taste-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load::<GenreIndex>(&dir, GENRES_FILE), GenreIndex::default());
        let g = index(5, 2);
        save(&dir, GENRES_FILE, &g);
        assert_eq!(load::<GenreIndex>(&dir, GENRES_FILE), g);
        std::fs::write(dir.join(RECS_FILE), b"not json").unwrap();
        assert_eq!(load::<RecsCache>(&dir, RECS_FILE), RecsCache::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
