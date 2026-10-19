//! Local music files: the persisted id registry and the serve-time checks.
//!
//! `local-files.json` is user-writable data, so nothing in it is trusted: every time a file is
//! handed out (ticket creation and every proxy request) its extension must be on the allowlist,
//! its canonical path must still be an existing regular file, and only that canonical path is read.
use super::{locked, Media, MediaError};
use crate::store::persist::{self, Read};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

const REGISTRY: &str = "local-files.json";

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

/// Read `local-files.json`. A missing file is an empty registry and a damaged one is moved aside
/// first; `None` means it exists but cannot be read now (antivirus, sync client), so it must not
/// be overwritten.
pub(super) async fn load_registry(root: &Path) -> Option<HashMap<String, PathBuf>> {
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || match persist::read_json(&root, REGISTRY) {
        Read::Missing => Some(HashMap::new()),
        Read::Value(v) => match serde_json::from_value(v) {
            Ok(files) => Some(files),
            Err(_) => persist::backup(&root.join(REGISTRY), "corrupt").map(|_| HashMap::new()),
        },
        Read::Corrupt { backup } => backup.map(|_| HashMap::new()),
        Read::Unreadable { .. } => None,
    })
    .await
    .ok()
    .flatten()
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
    /// Load the registry again when it was unreadable at start (caller holds `registry_lock`).
    /// False while it still cannot be read: a write then would replace the user's file.
    pub(super) async fn reload_if_unreadable(&self) -> bool {
        if !self.registry_unreadable.load(Ordering::SeqCst) {
            return true;
        }
        let Some(files) = load_registry(&self.root).await else {
            return false;
        };
        *locked(&self.files) = files;
        self.registry_unreadable.store(false, Ordering::SeqCst);
        true
    }

    /// Register many files with one write of the registry. Returns one id per path, in order.
    pub async fn register_local_batch(
        &self,
        paths: Vec<PathBuf>,
    ) -> Result<Vec<String>, MediaError> {
        let _one_writer = self.registry_lock.lock().await;
        if !self.reload_if_unreadable().await {
            return Err(MediaError::Io(format!("{REGISTRY} cannot be read")));
        }
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
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || persist::write_atomic(&root, REGISTRY, &bytes))
            .await
            .map_err(|e| MediaError::Io(e.to_string()))??;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ow-registry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn a_registry_that_cannot_be_read_is_not_treated_as_empty() {
        let d = temp();
        assert_eq!(load_registry(&d).await, Some(HashMap::new()), "missing");
        std::fs::write(d.join(REGISTRY), br#"{"local-a":"a.mp3"}"#).unwrap();
        assert_eq!(load_registry(&d).await.unwrap().len(), 1);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Held exclusively, like an antivirus scan during start-up.
            let lock = std::fs::File::options()
                .read(true)
                .share_mode(0)
                .open(d.join(REGISTRY))
                .unwrap();
            assert_eq!(
                load_registry(&d).await,
                None,
                "locked: must not be overwritten"
            );
            drop(lock);
        }
        // Damaged: set aside first, then an empty registry may be written.
        std::fs::write(d.join(REGISTRY), b"[1,2").unwrap();
        assert_eq!(load_registry(&d).await, Some(HashMap::new()));
        let kept = std::fs::read_dir(&d).unwrap().flatten().any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("local-files.json.corrupt-")
        });
        assert!(kept, "the damaged file is kept");
        let _ = std::fs::remove_dir_all(d);
    }
}
