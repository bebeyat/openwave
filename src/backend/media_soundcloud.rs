//! SoundCloud playback: yt-dlp fetches the whole song (it handles SoundCloud's HLS streams) into
//! the audio cache as `cache/sc-<id>.audio` + `.json`, and the proxy serves that file. The cache
//! is kept under [`CACHE_MAX_BYTES`], oldest files first.
use super::{soundcloud_id, ytdlp, Media, MediaError};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The default audio cache limit (`store::settings::DEFAULT_CACHE_LIMIT_MB`).
const CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const FETCH_LIMIT: Duration = Duration::from_secs(180);

/// Mime type for the container yt-dlp reports.
pub(super) fn mime_for(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "opus" | "ogg" => "audio/ogg",
        "webm" => "audio/webm",
        _ => "audio/mp4",
    }
}

/// yt-dlp output templates treat `%` as a field marker.
fn template_path(p: &Path) -> String {
    p.to_string_lossy().replace('%', "%%")
}

/// The cache files to delete so the `.audio` files stay within `max` bytes, oldest first;
/// `keep` (the song just fetched) is never chosen.
pub(super) fn over_limit(
    mut files: Vec<(std::time::SystemTime, u64, PathBuf)>,
    max: u64,
    keep: &Path,
) -> Vec<PathBuf> {
    let mut total: u64 = files.iter().map(|(_, n, _)| n).sum();
    files.sort();
    let mut out = vec![];
    for (_, n, p) in files {
        if total <= max {
            break;
        }
        if p != keep {
            total = total.saturating_sub(n);
            out.push(p);
        }
    }
    out
}

impl Media {
    /// The cached file of SoundCloud track `key`, fetched with yt-dlp first when missing.
    pub(super) async fn soundcloud_file(&self, key: &str) -> Result<(PathBuf, String), MediaError> {
        let id = soundcloud_id(key).ok_or(MediaError::InvalidTrackId)?;
        // Playback and a download of the same song share one fetch.
        let flight = self.flight(&format!("sc/{key}"));
        let _turn = flight.lock().await;
        let dir = self.root.join("cache");
        let audio = dir.join(format!("{key}.audio"));
        let meta = dir.join(format!("{key}.json"));
        if let Ok(bytes) = tokio::fs::read(&meta).await {
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            if audio.is_file() {
                return Ok((audio, v["mime"].as_str().unwrap_or("audio/mp4").to_owned()));
            }
        }
        let part = dir.join(format!("{key}.part"));
        let url = format!("https://api.soundcloud.com/tracks/{id}");
        let fetched = ytdlp::run_json(
            &[
                "--no-playlist",
                "--max-filesize",
                "512M",
                "-f",
                "bestaudio",
                "-o",
                &template_path(&part),
                "-j",
                "--no-simulate",
                &url,
            ],
            FETCH_LIMIT,
        )
        .await;
        let info = match fetched {
            Ok(v) if part.is_file() => v,
            Ok(_) => return Err(MediaError::Unavailable),
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(e);
            }
        };
        let mime = mime_for(info["ext"].as_str().unwrap_or(""));
        let record = serde_json::to_vec(&json!({ "mime": mime })).unwrap_or_default();
        tokio::fs::write(&meta, record).await?;
        tokio::fs::rename(&part, &audio).await?;
        self.trim_cache(&audio).await;
        Ok((audio, mime.to_owned()))
    }

    /// Keep the cached songs under [`CACHE_MAX_BYTES`] (files in use are skipped).
    async fn trim_cache(&self, keep: &Path) {
        let _guard = self.cache_lock.lock().await;
        let Ok(mut dir) = tokio::fs::read_dir(self.root.join("cache")).await else {
            return;
        };
        let mut files = vec![];
        while let Ok(Some(e)) = dir.next_entry().await {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "audio") {
                continue;
            }
            if let Ok(m) = e.metadata().await {
                files.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), p));
            }
        }
        for p in over_limit(files, CACHE_MAX_BYTES, keep) {
            if tokio::fs::remove_file(&p).await.is_ok() {
                let _ = tokio::fs::remove_file(p.with_extension("json")).await;
            }
        }
    }
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("media_soundcloud_tests.rs");
}
