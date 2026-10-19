//! Stream resolution, the local streaming proxy and the on-disk audio cache.
//!
//! Playback never talks to YouTube from the web view. [`Media::resolve`] finds an audio stream
//! (direct InnerTube call first, yt-dlp as fallback) and returns a URL on the loopback proxy
//! (`proxy.rs`), which only answers requests that carry the random per-launch token.
use super::{
    error::{Coded, MediaError},
    innertube,
    proxy::{self, Input, Ticket},
    ytdlp,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

#[path = "media_cache.rs"]
mod cache;
#[path = "media_download.rs"]
mod download;
pub use download::cancel_all as cancel_downloads;
#[path = "media_local.rs"]
mod local;
#[path = "media_resolve.rs"]
mod resolve;

pub(crate) use local::validate as validate_local;
pub use local::LOCAL_EXTENSIONS;

/// A ticket stays valid this long after its last use (every proxy request renews it), so a paused
/// or long-running track keeps working; unused tickets are dropped.
pub(super) const TICKET_IDLE_TTL: Duration = Duration::from_secs(3 * 3600);
const MAX_MEMOS: usize = 64;
/// Background lookups running at once (prefetch never floods the network).
const MAX_PREFETCH: usize = 4;
const MAX_TICKETS: usize = 48;

/// Debug builds only: `OPENWAVE_PERF_LOG=1` prints how each lookup was resolved and how long it took.
fn perf_log(key: &str, started: Instant, outcome: Result<&str, &str>) {
    if cfg!(debug_assertions) && std::env::var_os("OPENWAVE_PERF_LOG").is_some() {
        eprintln!(
            "PERF resolve {key} {outcome:?} {}ms",
            started.elapsed().as_millis()
        );
    }
}

fn retain_live(tickets: &mut HashMap<String, Ticket>, now: Instant, ttl: Duration) {
    tickets.retain(|_, t| now.saturating_duration_since(t.used) < ttl);
}

pub(super) fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A playable proxy URL for a track.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub url: String,
    pub mime: String,
    pub seconds: Option<f64>,
    ticket: String,
}

pub struct Media {
    /// Serialises cache eviction and clearing (downloads run outside it).
    cache_lock: tokio::sync::Mutex<()>,
    /// Serialises writes of `local-files.json` so an older snapshot never lands after a newer one.
    registry_lock: tokio::sync::Mutex<()>,
    /// `local-files.json` existed but could not be read at start: nothing may write it yet.
    registry_unreadable: std::sync::atomic::AtomicBool,
    pub(super) client: reqwest::Client,
    pub(super) tickets: Mutex<HashMap<String, Ticket>>,
    files: Mutex<HashMap<String, PathBuf>>,
    root: PathBuf,
    base: String,
    pub(super) port: u16,
    pub(super) secret: String,
    /// Resolved streams with the instant after which they must be looked up again.
    resolved: tokio::sync::Mutex<HashMap<String, (Instant, Resolved)>>,
    inflight: Mutex<HashSet<String>>,
    /// One lookup per track at a time: a click during a prefetch joins it instead of repeating it.
    flights: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Anonymous visitor id for the player endpoint, with the time it was fetched.
    visitor: tokio::sync::Mutex<Option<(Instant, String)>>,
}

pub fn video_key(key: &str) -> bool {
    key.len() == 11
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Only HTTPS media hosts of YouTube are ever fetched or followed.
pub(super) fn stream_url(s: &str) -> bool {
    reqwest::Url::parse(s).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port_or_known_default() == Some(443)
            && u.host_str()
                .is_some_and(|h| h.ends_with(".googlevideo.com"))
    })
}

impl Media {
    pub async fn open(root: PathBuf) -> Result<Arc<Self>, MediaError> {
        tokio::fs::create_dir_all(root.join("cache")).await?;
        // Loopback only: the proxy is never reachable from the network.
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let secret = uuid::Uuid::new_v4().to_string();
        let port = listener.local_addr()?.port();
        let base = format!("http://127.0.0.1:{port}/{secret}");
        // One client for everything: connections (and HTTP/2 sessions) to YouTube and the media
        // hosts stay open between songs, so a click does not pay a new TLS handshake.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(20))
            .pool_idle_timeout(Duration::from_secs(300))
            .tcp_nodelay(true)
            .redirect(reqwest::redirect::Policy::custom(|a| {
                if a.previous().len() < 4 && stream_url(a.url().as_str()) {
                    a.follow()
                } else {
                    a.stop()
                }
            }))
            .build()
            .map_err(|e| MediaError::Network(e.to_string()))?;
        let loaded = local::load_registry(&root).await;
        let registry_unreadable = std::sync::atomic::AtomicBool::new(loaded.is_none());
        let files = loaded.unwrap_or_default();
        let state = Arc::new(Self {
            cache_lock: tokio::sync::Mutex::new(()),
            registry_lock: tokio::sync::Mutex::new(()),
            registry_unreadable,
            client,
            tickets: Mutex::new(HashMap::new()),
            files: Mutex::new(files),
            root,
            base,
            port,
            secret,
            resolved: tokio::sync::Mutex::new(HashMap::new()),
            inflight: Mutex::new(HashSet::new()),
            flights: Mutex::new(HashMap::new()),
            visitor: tokio::sync::Mutex::new(None),
        });
        state.remove_stale_parts().await;
        let router = proxy::router(state.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("media proxy stopped: {e}");
            }
        });
        Ok(state)
    }

    fn grant(&self, input: Input, mime: String, seconds: Option<f64>) -> Resolved {
        let ticket = uuid::Uuid::new_v4().to_string();
        let mut tickets = locked(&self.tickets);
        retain_live(&mut tickets, Instant::now(), TICKET_IDLE_TTL);
        if tickets.len() >= MAX_TICKETS {
            if let Some(oldest) = tickets
                .iter()
                .min_by_key(|(_, t)| t.used)
                .map(|(k, _)| k.clone())
            {
                tickets.remove(&oldest);
            }
        }
        tickets.insert(
            ticket.clone(),
            Ticket {
                input,
                mime: mime.clone(),
                used: Instant::now(),
            },
        );
        Resolved {
            url: format!("{}/{ticket}", self.base),
            mime,
            seconds,
            ticket,
        }
    }

    /// The ticket for `key` if it is still valid at `now`; using it renews it (sliding expiry).
    pub(super) fn touch_ticket(&self, key: &str, now: Instant) -> Option<Ticket> {
        let mut tickets = locked(&self.tickets);
        let live = tickets
            .get(key)
            .is_some_and(|t| now.saturating_duration_since(t.used) < TICKET_IDLE_TTL);
        if !live {
            tickets.remove(key);
            return None;
        }
        let ticket = tickets.get_mut(key)?;
        ticket.used = now;
        Some(ticket.clone())
    }

    /// Folder of the small covers read from local files (`local_tags`).
    pub(crate) fn covers_dir(&self) -> PathBuf {
        self.root.join("covers")
    }

    /// `http://127.0.0.1:<port>/<secret>`: the proxy prefix for this launch.
    pub(crate) fn base_url(&self) -> &str {
        &self.base
    }

    /// The registered path of a local track id.
    pub(crate) fn local_path(&self, id: &str) -> Option<PathBuf> {
        locked(&self.files).get(id).cloned()
    }

    /// `quality`: `"economy"` (about 80 kbit/s) or `"high"`.
    pub async fn resolve(&self, key: &str, quality: &str) -> Result<Resolved, MediaError> {
        let started = Instant::now();
        let out = self.resolve_traced(key, quality).await;
        perf_log(
            key,
            started,
            out.as_ref().map(|(_, via)| *via).map_err(|e| e.code()),
        );
        out.map(|(r, _)| r)
    }

    /// Like [`Media::resolve`], also naming the path that produced the result.
    pub(super) async fn resolve_traced(
        &self,
        key: &str,
        quality: &str,
    ) -> Result<(Resolved, &'static str), MediaError> {
        if key.starts_with("local-")
            && self
                .registry_unreadable
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            let _one_writer = self.registry_lock.lock().await;
            self.reload_if_unreadable().await;
        }
        let local = locked(&self.files).get(key).cloned();
        if let Some(path) = local {
            let path = local::validate(&path).await?;
            return Ok((
                self.grant(Input::Local(path), "audio/mpeg".into(), None),
                "local",
            ));
        }
        if !video_key(key) {
            return Err(MediaError::InvalidTrackId);
        }
        // A downloaded track plays from disk, with or without a connection.
        if let Some((path, mime)) = self.downloaded_file(key).await {
            return Ok((self.grant(Input::File(path), mime, None), "download"));
        }
        let cache_key = format!("{quality}/{key}");
        if let Some(hit) = self.memo(&cache_key).await {
            return Ok((hit, "memo"));
        }
        if let Some(hit) = self.load_disk_cache(key).await {
            return Ok((hit, "disk"));
        }
        let flight = self.flight(&cache_key);
        let _turn = flight.lock().await;
        if let Some(hit) = self.memo(&cache_key).await {
            return Ok((hit, "memo"));
        }
        let (stream, via) = self.fetch_stream(key, quality).await?;
        let expires = stream.expires;
        let result = self.grant(Input::Web(stream.url), stream.mime, stream.seconds);
        self.remember(cache_key, expires, &result).await;
        Ok((result, via))
    }

    /// Drop the remembered stream of `key` so the next [`Media::resolve`] looks it up again.
    pub async fn forget(&self, key: &str, quality: &str) {
        self.resolved
            .lock()
            .await
            .remove(&format!("{quality}/{key}"));
    }

    /// A still-valid earlier result for `cache_key` (its ticket must also still exist).
    async fn memo(&self, cache_key: &str) -> Option<Resolved> {
        let resolved = self.resolved.lock().await;
        let (deadline, hit) = resolved.get(cache_key)?;
        (Instant::now() < *deadline && locked(&self.tickets).contains_key(&hit.ticket))
            .then(|| hit.clone())
    }

    fn flight(&self, cache_key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut flights = locked(&self.flights);
        flights.retain(|_, f| Arc::strong_count(f) > 1);
        flights.entry(cache_key.to_owned()).or_default().clone()
    }

    async fn load_disk_cache(&self, key: &str) -> Option<Resolved> {
        let dir = self.root.join("cache");
        let meta: serde_json::Value = serde_json::from_slice(
            &tokio::fs::read(dir.join(format!("{key}.json")))
                .await
                .ok()?,
        )
        .ok()?;
        let path = dir.join(format!("{key}.audio"));
        path.is_file().then(|| {
            self.grant(
                Input::File(path),
                meta["mime"].as_str().unwrap_or("audio/mp4").into(),
                None,
            )
        })
    }

    /// Resolve in the background so the later click finds it cached. Failures are ignored.
    pub fn prefetch(self: &Arc<Self>, key: &str, quality: &str) {
        if !video_key(key) {
            return;
        }
        let cache_key = format!("{quality}/{key}");
        {
            let mut inflight = locked(&self.inflight);
            if inflight.len() >= MAX_PREFETCH || !inflight.insert(cache_key.clone()) {
                return;
            }
        }
        let (me, key, quality) = (self.clone(), key.to_owned(), quality.to_owned());
        tokio::spawn(async move {
            let _ = me.resolve(&key, &quality).await;
            locked(&me.inflight).remove(&cache_key);
        });
    }

    async fn remember(&self, cache_key: String, expires: Option<u64>, result: &Resolved) {
        let now = Instant::now();
        let ttl = resolve::memo_ttl(expires, resolve::unix_now());
        let mut resolved = self.resolved.lock().await;
        resolved.retain(|_, (deadline, _)| *deadline > now);
        if resolved.len() >= MAX_MEMOS {
            if let Some(oldest) = resolved
                .iter()
                .min_by_key(|(_, (deadline, _))| *deadline)
                .map(|(k, _)| k.clone())
            {
                resolved.remove(&oldest);
            }
        }
        resolved.insert(cache_key, (now + ttl, result.clone()));
    }
}
