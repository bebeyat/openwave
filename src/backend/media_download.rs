//! Offline downloads: whole tracks saved under `downloads/` (never touched by cache cleanup).
//!
//! Layout: `downloads/<id>.audio` (the track), `downloads/<id>.json` (mime, size, when, and the
//! track's metadata so the list works offline) and, while a download runs, `downloads/<id>.part`.
//! The audio is fetched in range chunks, like the proxy does, to avoid googlevideo throttling.
//! [`Media::resolve`] plays a downloaded file before trying the network, so it works offline.
use super::{video_key, Media, MediaError};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;

/// Bumped by [`cancel_all`]; a running download that sees a newer value stops between chunks.
static CANCEL_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Stop every running download (they fail with "cancelled" and leave no file behind).
pub fn cancel_all() {
    CANCEL_EPOCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

fn cancelled_since(epoch: u64) -> bool {
    CANCEL_EPOCH.load(std::sync::atomic::Ordering::SeqCst) != epoch
}

/// Tracks being downloaded right now: a second download of the same id is refused instead of
/// writing the same `.part` file twice.
static IN_FLIGHT: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Removes the id from [`IN_FLIGHT`] when the download ends, however it ends.
struct FlightGuard(String);

impl FlightGuard {
    fn claim(key: &str) -> Option<Self> {
        let mut set = super::locked(&IN_FLIGHT);
        set.get_or_insert_with(HashSet::new)
            .insert(key.to_owned())
            .then(|| FlightGuard(key.to_owned()))
    }
}

impl Drop for FlightGuard {
    fn drop(&mut self) {
        if let Some(set) = super::locked(&IN_FLIGHT).as_mut() {
            set.remove(&self.0);
        }
    }
}

/// Bytes asked for per request.
const CHUNK: u64 = 4 * 1024 * 1024;
/// A track larger than this is refused (a stream that never ends must not fill the disk).
const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// One downloaded track as listed: its stored metadata, size and when it was saved.
#[derive(Clone, Debug, PartialEq)]
pub struct DownloadRecord {
    pub meta: Value,
    pub bytes: u64,
    pub saved_ms: u64,
}

fn net(e: reqwest::Error) -> MediaError {
    MediaError::Network(e.without_url().to_string())
}

/// `Content-Range: bytes a-b/total` as (a, b, total).
fn content_range(resp: &reqwest::Response) -> Option<(u64, u64, u64)> {
    let v = resp.headers().get("content-range")?.to_str().ok()?;
    let rest = v.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (a, b) = range.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?, total.parse().ok()?))
}

impl Media {
    fn downloads_dir(&self) -> PathBuf {
        self.root.join("downloads")
    }

    /// The downloaded file and its mime type, when `key` is downloaded.
    pub(super) async fn downloaded_file(&self, key: &str) -> Option<(PathBuf, String)> {
        if !video_key(key) {
            return None;
        }
        let dir = self.downloads_dir();
        let path = dir.join(format!("{key}.audio"));
        if !tokio::fs::metadata(&path).await.is_ok_and(|m| m.is_file()) {
            return None;
        }
        let meta: Value = tokio::fs::read(dir.join(format!("{key}.json")))
            .await
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        Some((
            path,
            meta["mime"].as_str().unwrap_or("audio/mp4").to_owned(),
        ))
    }

    /// Download `key` completely and keep it with `track` (its metadata). Returns the size.
    /// Already downloaded tracks return at once.
    pub async fn download(
        &self,
        key: &str,
        quality: &str,
        track: Value,
    ) -> Result<u64, MediaError> {
        if !video_key(key) {
            return Err(MediaError::InvalidTrackId);
        }
        let dir = self.downloads_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let audio = dir.join(format!("{key}.audio"));
        if let Ok(m) = tokio::fs::metadata(&audio).await {
            return Ok(m.len());
        }
        let Some(_flight) = FlightGuard::claim(key) else {
            return Err(MediaError::Io("download already running".into()));
        };
        let (stream, _) = self.fetch_stream(key, quality).await?;
        let part = dir.join(format!("{key}.part"));
        let written = self.fetch_to(&stream.url, &part).await;
        let bytes = match written {
            Ok(n) => n,
            Err(e) => {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(e);
            }
        };
        let record = json!({
            "mime": stream.mime,
            "bytes": bytes,
            "saved_ms": super::resolve::unix_now() * 1000,
            "track": track,
        });
        let meta = serde_json::to_vec(&record).map_err(|e| MediaError::Io(e.to_string()))?;
        tokio::fs::write(dir.join(format!("{key}.json")), meta).await?;
        tokio::fs::rename(&part, &audio).await?;
        Ok(bytes)
    }

    /// Stream `url` into `path` chunk by chunk; the byte count on success.
    async fn fetch_to(&self, url: &str, path: &PathBuf) -> Result<u64, MediaError> {
        let epoch = CANCEL_EPOCH.load(std::sync::atomic::Ordering::SeqCst);
        let mut file = tokio::fs::File::create(path).await?;
        let mut next = 0_u64;
        loop {
            if cancelled_since(epoch) {
                return Err(MediaError::Io("cancelled".into()));
            }
            let resp = self
                .client
                .get(url)
                .header("Range", format!("bytes={next}-{}", next + CHUNK - 1))
                .send()
                .await
                .map_err(net)?;
            let status = resp.status().as_u16();
            // Asking past the end (size an exact multiple of the chunk, no Content-Range seen).
            if status == 416 && next > 0 {
                break;
            }
            // A server that ignores Range after the first chunk would append the whole file.
            if status != 206 && (status != 200 || next > 0) {
                return Err(MediaError::BadResponse);
            }
            let total = content_range(&resp).map(|(_, _, t)| t);
            let whole = status == 200;
            let mut body = resp.bytes_stream();
            let mut got = 0_u64;
            while let Some(chunk) = futures_util::StreamExt::next(&mut body).await {
                if cancelled_since(epoch) {
                    return Err(MediaError::Io("cancelled".into()));
                }
                let chunk = chunk.map_err(net)?;
                got += chunk.len() as u64;
                if next + got > MAX_BYTES {
                    return Err(MediaError::BadResponse);
                }
                file.write_all(&chunk).await?;
            }
            next += got;
            if total.is_some_and(|t| next > t) {
                return Err(MediaError::BadResponse);
            }
            let done = whole || got == 0 || total.is_some_and(|t| next == t);
            if done {
                break;
            }
        }
        file.flush().await?;
        if next == 0 {
            return Err(MediaError::BadResponse);
        }
        Ok(next)
    }

    /// Every downloaded track, newest first.
    pub async fn downloads(&self) -> Result<Vec<DownloadRecord>, MediaError> {
        let dir = self.downloads_dir();
        let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
            return Ok(vec![]);
        };
        let mut out = vec![];
        while let Some(e) = entries.next_entry().await? {
            let p = e.path();
            let Some(key) = p
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|k| video_key(k))
            else {
                continue;
            };
            if p.extension().is_none_or(|x| x != "json")
                || !dir.join(format!("{key}.audio")).is_file()
            {
                continue;
            }
            let Some(v) = tokio::fs::read(&p)
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            out.push(DownloadRecord {
                meta: v["track"].clone(),
                bytes: v["bytes"].as_u64().unwrap_or(0),
                saved_ms: v["saved_ms"].as_u64().unwrap_or(0),
            });
        }
        out.sort_by_key(|r| std::cmp::Reverse(r.saved_ms));
        Ok(out)
    }

    /// Delete one download (a file in use is left for the next try and reported).
    pub async fn remove_download(&self, key: &str) -> Result<(), MediaError> {
        if !video_key(key) {
            return Err(MediaError::InvalidTrackId);
        }
        let dir = self.downloads_dir();
        for ext in ["audio", "json", "part"] {
            match tokio::fs::remove_file(dir.join(format!("{key}.{ext}"))).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };

    const BODY_LEN: usize = 10 * 1024 * 1024 + 123;

    fn body() -> Vec<u8> {
        (0..BODY_LEN).map(|i| (i % 251) as u8).collect()
    }

    /// A server that only answers ranges, like googlevideo.
    async fn serve() -> String {
        let app = Router::new().route(
            "/a",
            get(|headers: HeaderMap| async move {
                let data = body();
                let range = headers["range"]
                    .to_str()
                    .unwrap()
                    .trim_start_matches("bytes=")
                    .to_owned();
                let (a, b) = range.split_once('-').unwrap();
                let a: usize = a.parse().unwrap();
                let b: usize = b.parse::<usize>().unwrap().min(data.len() - 1);
                let mut h = HeaderMap::new();
                h.insert(
                    "content-range",
                    format!("bytes {a}-{b}/{}", data.len()).parse().unwrap(),
                );
                (StatusCode::PARTIAL_CONTENT, h, data[a..=b].to_vec()).into_response()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        base
    }

    #[tokio::test]
    async fn chunked_fetch_writes_the_whole_file() {
        let root = std::env::temp_dir().join(format!("ow-dl-{}", uuid::Uuid::new_v4()));
        let media = Media::open(root.clone()).await.unwrap();
        let base = serve().await;
        let path = root.join("x.part");
        let n = media.fetch_to(&format!("{base}/a"), &path).await.unwrap();
        assert_eq!(n as usize, BODY_LEN);
        assert_eq!(std::fs::read(&path).unwrap(), body());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A server that ignores Range and always sends the whole body with 200.
    async fn serve_whole() -> String {
        let app = Router::new().route(
            "/a",
            get(|| async { (StatusCode::OK, body()).into_response() }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        base
    }

    #[tokio::test]
    async fn a_full_200_answer_is_taken_whole_once() {
        let root = std::env::temp_dir().join(format!("ow-dl-{}", uuid::Uuid::new_v4()));
        let media = Media::open(root.clone()).await.unwrap();
        let base = serve_whole().await;
        let path = root.join("w.part");
        let n = media.fetch_to(&format!("{base}/a"), &path).await.unwrap();
        assert_eq!(n as usize, BODY_LEN, "not appended twice");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn the_same_track_is_never_downloaded_twice_at_once() {
        let first = FlightGuard::claim("dQw4w9WgXcQ");
        assert!(first.is_some());
        assert!(FlightGuard::claim("dQw4w9WgXcQ").is_none());
        drop(first);
        assert!(FlightGuard::claim("dQw4w9WgXcQ").is_some());
    }

    #[tokio::test]
    async fn listing_and_removal_use_only_complete_downloads() {
        let root = std::env::temp_dir().join(format!("ow-dl-{}", uuid::Uuid::new_v4()));
        let media = Media::open(root.clone()).await.unwrap();
        let dir = root.join("downloads");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dQw4w9WgXcQ.audio"), b"abc").unwrap();
        std::fs::write(
            dir.join("dQw4w9WgXcQ.json"),
            br#"{"mime":"audio/webm","bytes":3,"saved_ms":5,"track":{"id":"dQw4w9WgXcQ"}}"#,
        )
        .unwrap();
        // Metadata without audio (an interrupted download) is not listed.
        std::fs::write(dir.join("aaaaaaaaaaa.json"), br#"{"bytes":1}"#).unwrap();
        let list = media.downloads().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].bytes, list[0].saved_ms), (3, 5));
        assert_eq!(list[0].meta["id"], "dQw4w9WgXcQ");
        let (path, mime) = media.downloaded_file("dQw4w9WgXcQ").await.unwrap();
        assert!(path.ends_with("dQw4w9WgXcQ.audio") && mime == "audio/webm");
        media.remove_download("dQw4w9WgXcQ").await.unwrap();
        assert!(media.downloads().await.unwrap().is_empty());
        assert!(media.downloaded_file("dQw4w9WgXcQ").await.is_none());
        assert_eq!(
            media.remove_download("../x").await,
            Err(MediaError::InvalidTrackId)
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
