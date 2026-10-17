//! On-disk cache of the "Sana özel karışımlar" so Home can paint them instantly (and offline)
//! before the background refresh finishes. Stored as `home_mixes.json` in the data directory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::{Cover, Track};
use crate::store::persist::write_atomic;

pub const FILE: &str = "home_mixes.json";
/// Mixes older than this are rebuilt in the background.
pub const MAX_AGE_MS: u64 = 6 * 60 * 60 * 1000;

/// Similar songs for one seed track the user played or liked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mix {
    pub seed_id: String,
    pub artist: String,
    pub cover: Option<Cover>,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MixCache {
    pub saved_ms: u64,
    pub mixes: Vec<Mix>,
}

impl MixCache {
    /// True when the cached mixes were built from exactly `seed_ids` less than [`MAX_AGE_MS`] ago.
    pub fn is_fresh(&self, seed_ids: &[String], now_ms: u64) -> bool {
        let ids: Vec<&str> = self.mixes.iter().map(|m| m.seed_id.as_str()).collect();
        let want: Vec<&str> = seed_ids.iter().map(String::as_str).collect();
        !self.mixes.is_empty() && ids == want && now_ms.saturating_sub(self.saved_ms) < MAX_AGE_MS
    }
}

/// Missing or unreadable files give an empty cache (the cache is only a convenience).
pub fn load(dir: &Path) -> MixCache {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, cache: &MixCache) {
    if let Ok(bytes) = serde_json::to_vec(cache) {
        // Best effort: a failed write only means the next start rebuilds the mixes.
        let _ = write_atomic(dir, FILE, &bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix(id: &str) -> Mix {
        Mix {
            seed_id: id.into(),
            tracks: vec![Track::default()],
            ..Default::default()
        }
    }

    fn cache(ids: &[&str], saved_ms: u64) -> MixCache {
        MixCache {
            saved_ms,
            mixes: ids.iter().map(|i| mix(i)).collect(),
        }
    }

    #[test]
    fn fresh_only_for_same_seeds_and_recent() {
        let c = cache(&["a", "b"], 1_000);
        let same = vec!["a".to_string(), "b".to_string()];
        assert!(c.is_fresh(&same, 1_000 + MAX_AGE_MS - 1));
        assert!(!c.is_fresh(&same, 1_000 + MAX_AGE_MS));
        assert!(!c.is_fresh(&["a".to_string()], 1_001));
        assert!(!c.is_fresh(&["b".to_string(), "a".to_string()], 1_001));
        assert!(!MixCache::default().is_fresh(&[], 0));
    }

    #[test]
    fn round_trips_through_disk_and_survives_garbage() {
        let dir = std::env::temp_dir().join(format!("ow-home-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir), MixCache::default());
        let c = cache(&["a"], 5);
        save(&dir, &c);
        assert_eq!(load(&dir), c);
        std::fs::write(dir.join(FILE), b"not json").unwrap();
        assert_eq!(load(&dir), MixCache::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
