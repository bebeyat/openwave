//! The network and disk side of the palette extraction: the image-host allow-list, the bounded
//! download (https, three image host families, no redirects, size cap) and the small JSON cache.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::palette::{from_image_bytes, Palette};

const MAX_DOWNLOAD: usize = 1_500_000;
pub(super) const CACHE_ENTRIES: usize = 256;
pub(super) const CACHE_VERSION: u32 = 2;

/// Same rule as the catalog's allow-list: https, no credentials, the YouTube image host families
/// plus SoundCloud's artwork host.
pub fn allowed_image_url(s: &str) -> bool {
    reqwest::Url::parse(s).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str().is_some_and(|h| {
                [
                    "ytimg.com",
                    "ggpht.com",
                    "googleusercontent.com",
                    "sndcdn.com",
                ]
                .iter()
                .any(|d| h == *d || h.ends_with(&format!(".{d}")))
            })
    })
}

/// Asks image hosts that support it for a small rendition (`=w544-h544-l90-rj` becomes 128px).
pub fn small_url(url: &str) -> String {
    if let Some(i) = url.rfind("=w") {
        let tail = &url[i + 2..];
        let looks_sized = tail.chars().next().is_some_and(|c| c.is_ascii_digit());
        if looks_sized && !tail.contains('/') && !tail.contains('?') {
            return format!("{}=w128-h128-l90-rj", &url[..i]);
        }
    }
    url.to_owned()
}

fn http() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(6))
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) async fn download(url: &str) -> Result<Vec<u8>, String> {
    if !allowed_image_url(url) {
        return Err("image host not allowed".into());
    }
    let resp = http()?.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("http {}", resp.status()));
    }
    if resp
        .content_length()
        .is_some_and(|n| n as usize > MAX_DOWNLOAD)
    {
        return Err("image too large".into());
    }
    let mut body = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        if body.len() + chunk.len() > MAX_DOWNLOAD {
            return Err("image too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

// ---- disk cache ------------------------------------------------------------------------------

#[derive(Default, Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    /// Oldest first; bounded to `CACHE_ENTRIES`.
    entries: VecDeque<(String, Palette)>,
}

/// Small JSON cache keyed by a hash of the cover URL.
pub struct Cache {
    path: PathBuf,
    file: CacheFile,
}

pub fn url_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

impl Cache {
    pub fn open(path: PathBuf) -> Self {
        let file = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<CacheFile>(&b).ok())
            .filter(|f| f.version == CACHE_VERSION)
            .unwrap_or(CacheFile {
                version: CACHE_VERSION,
                entries: VecDeque::new(),
            });
        Self { path, file }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.file.entries.len()
    }

    pub fn get(&self, key: &str) -> Option<Palette> {
        self.file
            .entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, p)| p.clone())
    }

    /// Inserts (replacing a same-key entry), evicts the oldest beyond the bound, writes the file.
    pub fn put(&mut self, key: String, palette: Palette) {
        self.file.entries.retain(|(k, _)| *k != key);
        self.file.entries.push_back((key, palette));
        while self.file.entries.len() > CACHE_ENTRIES {
            self.file.entries.pop_front();
        }
        self.save();
    }

    fn save(&self) {
        let Ok(json) = serde_json::to_vec(&self.file) else {
            return;
        };
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, &self.path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn global_cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::open(cache_path(&super::data_dir()))))
}

fn cache_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cache").join("palette.json")
}

/// Palette of the cover at `url`, from the disk cache or the network. `Err` means "use the
/// default look" (offline, host not allowed, undecodable image).
pub async fn palette_for(url: &str) -> Result<Palette, String> {
    let key = url_key(url);
    if let Some(p) = global_cache().lock().ok().and_then(|c| c.get(&key)) {
        return Ok(p);
    }
    let bytes = download(&small_url(url)).await?;
    let palette = tokio::task::spawn_blocking(move || from_image_bytes(&bytes))
        .await
        .map_err(|e| e.to_string())??;
    if let Ok(mut c) = global_cache().lock() {
        c.put(key, palette.clone());
    }
    Ok(palette)
}
