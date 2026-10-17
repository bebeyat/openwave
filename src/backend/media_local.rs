//! Local music files: the persisted id registry and the serve-time checks.
//!
//! `local-files.json` is user-writable data, so nothing in it is trusted: every time a file is
//! handed out (ticket creation and every proxy request) its extension must be on the allowlist,
//! its canonical path must still be an existing regular file, and only that canonical path is read.
use super::{locked, Media, MediaError};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

/// Extensions of files that may be played from disk (also the file picker's filter).
pub const LOCAL_EXTENSIONS: [&str; 7] = ["mp3", "m4a", "aac", "flac", "wav", "ogg", "opus"];

fn allowed_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| LOCAL_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The canonical path of a playable local file, or `Unavailable`.
pub(crate) async fn validate(path: &Path) -> Result<PathBuf, MediaError> {
    if !allowed_extension(path) {
        return Err(MediaError::Unavailable);
    }
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| MediaError::Unavailable)?;
    let is_file = tokio::fs::metadata(&canonical)
        .await
        .is_ok_and(|m| m.is_file());
    // A link may point somewhere with another extension.
    if is_file && allowed_extension(&canonical) {
        Ok(canonical)
    } else {
        Err(MediaError::Unavailable)
    }
}

/// Read `local-files.json`; a missing or damaged file means an empty registry.
pub(super) async fn load_registry(root: &Path) -> HashMap<String, PathBuf> {
    tokio::fs::read(root.join("local-files.json"))
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Serialize the registry. Paths are written as (lossy) text, so a path with unpaired UTF-16
/// surrogates cannot make serialization fail; such a file plays this session and is simply not
/// found after a restart.
fn snapshot(files: &HashMap<String, PathBuf>) -> Result<Vec<u8>, MediaError> {
    let wire: BTreeMap<&str, String> = files
        .iter()
        .map(|(id, p)| (id.as_str(), p.to_string_lossy().into_owned()))
        .collect();
    serde_json::to_vec(&wire).map_err(|e| MediaError::Io(e.to_string()))
}

impl Media {
    /// Register many files with one write of the registry. Returns one id per path, in order.
    pub async fn register_local_batch(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<Vec<String>, MediaError> {
        let _one_writer = self.registry_lock.lock().await;
        let (ids, bytes) = {
            let mut files = locked(&self.files);
            let mut by_path: HashMap<PathBuf, String> =
                files.iter().map(|(k, p)| (p.clone(), k.clone())).collect();
            let mut ids = Vec::with_capacity(paths.len());
            for path in paths {
                let id = by_path
                    .entry(path.clone())
                    .or_insert_with(|| format!("local-{}", uuid::Uuid::new_v4()))
                    .clone();
                files.insert(id.clone(), path);
                ids.push(id);
            }
            let bytes = snapshot(&files)?;
            (ids, bytes)
        };
        let tmp = self
            .root
            .join(format!("local-files.{}.tmp", uuid::Uuid::new_v4()));
        let written = match tokio::fs::write(&tmp, bytes).await {
            Ok(()) => tokio::fs::rename(&tmp, self.root.join("local-files.json")).await,
            Err(e) => Err(e),
        };
        if let Err(e) = written {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e.into());
        }
        Ok(ids)
    }
}
