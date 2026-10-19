//! The on-disk audio cache: statistics and cleanup (stale `.part` files, clear).
//!
//! Layout: `cache/<id>.audio` (the track), `cache/<id>.json` (its mime type) and, while a download
//! runs, `cache/<id>.part`. Files that cannot be removed right now (Windows locks files that are
//! being served) are skipped and retried on the next pass; they never abort a pass.
use super::{video_key, Media, MediaError};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

/// A `.part` file older than this is the leftover of a crashed or killed download.
const STALE_PART_AGE: Duration = Duration::from_secs(3600);

impl Media {
    async fn cache_files(
        &self,
        ext: &str,
    ) -> Result<Vec<(PathBuf, std::fs::Metadata)>, MediaError> {
        self.files_in("cache", ext).await
    }

    async fn files_in(
        &self,
        sub: &str,
        ext: &str,
    ) -> Result<Vec<(PathBuf, std::fs::Metadata)>, MediaError> {
        let mut dir = tokio::fs::read_dir(self.root.join(sub)).await?;
        let mut out = vec![];
        while let Some(e) = dir.next_entry().await? {
            if e.path().extension().is_some_and(|s| s == ext) {
                if let Ok(meta) = e.metadata().await {
                    out.push((e.path(), meta));
                }
            }
        }
        Ok(out)
    }

    /// (bytes on disk, number of cached tracks). Bytes include unfinished `.part` downloads, which
    /// also occupy the folder; the track count does not.
    pub async fn stats(&self) -> Result<(u64, u64), MediaError> {
        let files = self.cache_files("audio").await?;
        let parts = self.cache_files("part").await?;
        let bytes = files.iter().chain(&parts).map(|(_, m)| m.len()).sum();
        Ok((bytes, files.len() as u64))
    }

    /// Remove `.part` files left behind by a crash (older than an hour) in the cache and the
    /// offline downloads. Returns how many went.
    pub(super) async fn remove_stale_parts(&self) -> u64 {
        let mut parts = vec![];
        for sub in ["cache", "downloads"] {
            parts.extend(self.files_in(sub, "part").await.unwrap_or_default());
        }
        let mut removed = 0;
        for (path, meta) in parts {
            let age = meta
                .modified()
                .ok()
                .and_then(|m| SystemTime::now().duration_since(m).ok());
            if age.is_some_and(|a| a >= STALE_PART_AGE)
                && tokio::fs::remove_file(&path).await.is_ok()
            {
                removed += 1;
            }
        }
        removed
    }

    /// Delete cached audio only; unrelated files in the folder are left alone and files that are
    /// in use are skipped.
    pub async fn clear(&self) -> Result<(), MediaError> {
        let _guard = self.cache_lock.lock().await;
        let mut dir = tokio::fs::read_dir(self.root.join("cache")).await?;
        while let Some(e) = dir.next_entry().await? {
            let p = e.path();
            let ours = p
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(video_key)
                && p.extension()
                    .is_some_and(|s| s == "audio" || s == "json" || s == "part");
            if ours && e.file_type().await.is_ok_and(|t| t.is_file()) {
                let _ = tokio::fs::remove_file(p).await;
            }
        }
        Ok(())
    }
}
