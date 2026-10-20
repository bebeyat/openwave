//! Lyrics kept on disk (`lyrics/<hash>.json`) so a song seen once needs no request again, plus
//! its translations. Bounded to [`MAX_FILES`]; the oldest files go first.
use crate::model::Lyrics;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const DIR: &str = "lyrics";
const MAX_FILES: usize = 500;

/// What is stored for one song: its lyrics and translated lines per target language.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub lyrics: Option<Lyrics>,
    pub translations: BTreeMap<String, Vec<String>>,
}

/// Cache file names: 16 lowercase hex digits plus `.json` (or `.tmp` while written).
pub fn is_cache_file(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        matches!(ext, "json" | "tmp") && super::local_tags::valid_cover_hash(stem)
    })
}

/// Stable key of a lookup (title, artist, album and whole seconds, case-insensitive).
pub fn key(title: &str, artist: &str, album: Option<&str>, duration_secs: Option<f64>) -> String {
    let secs = duration_secs.map_or(0, |s| s.round() as u64);
    let text = format!(
        "{}\n{}\n{}\n{secs}",
        title.trim().to_lowercase(),
        artist.trim().to_lowercase(),
        album.unwrap_or("").trim().to_lowercase()
    );
    let digest = Sha256::digest(text.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn cache_key(q: &super::lyrics::LyricsQuery<'_>) -> String {
    key(q.title, q.artist, q.album, q.duration_secs)
}

fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.json"))
}

pub fn get(dir: &Path, key: &str) -> Option<Entry> {
    let bytes = std::fs::read(path(dir, key)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Store `entry` (atomic replace), then trim the folder to [`MAX_FILES`].
pub fn put(dir: &Path, key: &str, entry: &Entry) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
    let tmp = dir.join(format!("{key}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path(dir, key))?;
    evict(dir, MAX_FILES);
    Ok(())
}

/// Delete the oldest cache files beyond `keep`.
pub(crate) fn evict(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| is_cache_file(&e.file_name().to_string_lossy()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= keep {
        return;
    }
    files.sort();
    for (_, p) in &files[..files.len() - keep] {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("lyrics_cache_tests.rs");
}
