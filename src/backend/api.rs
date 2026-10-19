//! High-level async API used by the UI. Everything runs on a dedicated tokio runtime so it works
//! regardless of the UI executor. Errors are typed (`ApiError`, see `error.rs`).
use super::{
    catalog::Catalog,
    convert,
    error::{ApiError, ApiResult, CatalogError, ImportError, ShareError},
    import::{self, Source},
    import_flow::{self, ImportResult, PreparedImport},
    lyrics::{self, LyricsQuery},
    media::Media,
    share, ytdlp,
};
use crate::model::*;
use serde_json::Value;
use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

pub use super::import::SpotifyTrack;

const CATALOG_TIMEOUT: Duration = Duration::from_secs(25);

struct Backend {
    rt: tokio::runtime::Runtime,
    catalog: Catalog,
    media: Arc<Media>,
    root: PathBuf,
}

static BACKEND: OnceLock<Backend> = OnceLock::new();

/// Per-user data directory (`%APPDATA%\OpenWave` on Windows, `OPENWAVE_DATA_DIR` overrides).
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("OPENWAVE_DATA_DIR") {
        return PathBuf::from(p);
    }
    // `%APPDATA%\OpenWave` (roaming app data), the folder the docs and the uninstaller name.
    directories::BaseDirs::new()
        .map(|d| d.data_dir().join("OpenWave"))
        .unwrap_or_else(|| std::env::temp_dir().join("OpenWave"))
}

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

/// Start the runtime, the media proxy and the catalog client. Call once before the UI starts.
pub fn init() -> ApiResult<()> {
    let root = data_dir();
    std::fs::create_dir_all(&root).map_err(internal)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(internal)?;
    let media = rt.block_on(Media::open(root.clone()))?;
    let backend = Backend {
        rt,
        catalog: Catalog::new(),
        media,
        root,
    };
    if BACKEND.set(backend).is_ok() {
        if let Ok(b) = backend_ref() {
            b.rt.spawn(b.catalog.warm());
            b.rt.spawn(b.media.clone().warm());
        }
    }
    Ok(())
}

fn backend_ref() -> ApiResult<&'static Backend> {
    BACKEND
        .get()
        .ok_or_else(|| ApiError::Internal("backend::init() must run first".into()))
}

/// Run a future on the backend runtime and await its result from any executor.
async fn run<T: Send + 'static>(fut: impl Future<Output = T> + Send + 'static) -> ApiResult<T> {
    backend_ref()?.rt.spawn(fut).await.map_err(internal)
}

async fn query(section: &'static str, term: String, key: String) -> ApiResult<Value> {
    let b = backend_ref()?;
    run(async move {
        tokio::time::timeout(CATALOG_TIMEOUT, b.catalog.query(section, &term, &key))
            .await
            .map_err(|_| CatalogError::Network("timeout".into()))?
    })
    .await?
    .map_err(ApiError::from)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchKind {
    #[default]
    All,
    Track,
    Artist,
    Album,
    Playlist,
}

impl SearchKind {
    fn wants(self, other: SearchKind) -> bool {
        self == SearchKind::All || self == other
    }
}

async fn section(on: bool, name: &'static str, term: &str) -> ApiResult<Value> {
    if on {
        query(name, term.to_owned(), String::new()).await
    } else {
        Ok(Value::Null)
    }
}

/// Search the catalog. `All` tolerates a partly failing catalog; a single kind reports its error.
pub async fn search(kind: SearchKind, term: &str) -> ApiResult<SearchResults> {
    let term = term.trim();
    if term.is_empty() {
        return Ok(SearchResults::default());
    }
    let (t, ar, al, pl) = futures_util::join!(
        section(kind.wants(SearchKind::Track), "tracks", term),
        section(kind.wants(SearchKind::Artist), "artists", term),
        section(kind.wants(SearchKind::Album), "albums", term),
        section(kind.wants(SearchKind::Playlist), "lists", term),
    );
    if kind != SearchKind::All {
        for r in [&t, &ar, &al, &pl] {
            if let Err(e) = r {
                return Err(e.clone());
            }
        }
    }
    let list = |v: ApiResult<Value>| v.map(|v| convert::entries(&v, "items")).unwrap_or_default();
    Ok(SearchResults {
        tracks: list(t).iter().map(convert::track).collect(),
        artists: list(ar).into_iter().map(convert::artist_item).collect(),
        albums: list(al).into_iter().map(convert::album_item).collect(),
        playlists: list(pl).into_iter().map(convert::playlist_item).collect(),
    })
}

pub async fn artist(id: &str) -> ApiResult<ArtistDetails> {
    let v = query("artist", String::new(), id.to_owned()).await?;
    Ok(ArtistDetails {
        id: id.into(),
        name: v["name"].as_str().unwrap_or("").into(),
        bio: v["description"].as_str().unwrap_or("").to_owned(),
        cover: convert::cover(v["artwork"].as_str().unwrap_or("")),
        tracks: convert::entries(&v, "items")
            .iter()
            .map(convert::track)
            .collect(),
        albums: convert::entries(&v, "releases")
            .into_iter()
            .map(convert::album_item)
            .collect(),
    })
}

pub async fn collection(kind: CollectionKind, id: &str) -> ApiResult<CollectionDetails> {
    let section = if kind == CollectionKind::Album {
        "album"
    } else {
        "list"
    };
    let v = query(section, String::new(), id.to_owned()).await?;
    let tracks: Vec<Track> = convert::entries(&v, "items")
        .iter()
        .map(convert::track)
        .collect();
    let author = tracks.first().map(Track::artists_label).unwrap_or_default();
    Ok(CollectionDetails {
        kind,
        id: id.into(),
        title: v["name"].as_str().unwrap_or("").into(),
        author,
        description: v["description"].as_str().unwrap_or("").to_owned(),
        cover: convert::cover(v["artwork"].as_str().unwrap_or("")),
        tracks,
    })
}

/// Similar tracks ("autoplay similar") for a track id.
pub async fn radio(track_id: &str) -> ApiResult<Vec<Track>> {
    let v = query("radio", String::new(), track_id.to_owned()).await?;
    Ok(convert::entries(&v, "items")
        .iter()
        .map(convert::track)
        .collect())
}

/// Discovery rows for language `hl` (`tr`, `pt-BR`…) and country `gl` (`TR`; `None` lets the
/// server use the connection's country), or only the rows of one mood `chip`.
pub async fn discover(hl: String, gl: Option<String>, chip: Option<String>) -> ApiResult<Discover> {
    use super::catalog::discover;
    let raw = run(async move {
        tokio::time::timeout(
            CATALOG_TIMEOUT,
            discover::discover(&hl, gl.as_deref(), chip.as_deref()),
        )
        .await
        .map_err(|_| CatalogError::Network("timeout".into()))?
    })
    .await?
    .map_err(ApiError::from)?;
    let shelves = raw
        .shelves
        .into_iter()
        .map(|s| Shelf {
            title: s.title,
            items: match s.kind {
                "track" => ShelfItems::Tracks(s.items.iter().map(convert::track).collect()),
                "album" => {
                    ShelfItems::Albums(s.items.into_iter().map(convert::album_item).collect())
                }
                "list" => {
                    ShelfItems::Playlists(s.items.into_iter().map(convert::playlist_item).collect())
                }
                _ => ShelfItems::Artists(s.items.into_iter().map(convert::artist_item).collect()),
            },
        })
        .collect();
    let chips = raw
        .chips
        .into_iter()
        .map(|c| MoodChip {
            label: c.label,
            params: c.params,
        })
        .collect();
    Ok(Discover { chips, shelves })
}

fn links(v: &Value) -> Vec<CatalogLink> {
    v["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| serde_json::from_value::<CatalogLink>(i.clone()).ok())
                .filter(|l| !l.name.is_empty() && !l.key.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The catalog's genres (for the user's region), in catalog order.
pub async fn genres() -> ApiResult<Vec<CatalogLink>> {
    Ok(links(&query("genres", String::new(), String::new()).await?))
}

/// Playlists of one genre, in catalog order; open one with [`collection`] (`Playlist`).
pub async fn genre_playlists(genre_key: &str) -> ApiResult<Vec<CatalogLink>> {
    Ok(links(
        &query("genre", String::new(), genre_key.to_owned()).await?,
    ))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Economy,
    #[default]
    High,
}

impl Quality {
    fn as_str(self) -> &'static str {
        match self {
            Quality::Economy => "economy",
            Quality::High => "high",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    /// Loopback proxy URL for the `<audio>` element.
    pub url: String,
    pub mime: String,
    pub seconds: Option<f64>,
}

/// A downloaded track: its saved metadata, size on disk and when it was saved (Unix ms).
#[derive(Clone, Debug, PartialEq)]
pub struct Downloaded {
    pub track: Track,
    pub bytes: u64,
    pub saved_ms: u64,
}

/// Download `track` completely for offline listening; returns its size. Local files and
/// already downloaded tracks return at once.
pub async fn download_track(track: &Track, quality: Quality) -> ApiResult<u64> {
    let b = backend_ref()?;
    let id = track.id.to_string();
    let meta = serde_json::to_value(track).map_err(internal)?;
    Ok(run(async move { b.media.download(&id, quality.as_str(), meta).await }).await??)
}

/// Every downloaded track, newest first (entries whose metadata cannot be read are skipped).
pub async fn downloads() -> ApiResult<Vec<Downloaded>> {
    let b = backend_ref()?;
    let records = run(async move { b.media.downloads().await }).await??;
    Ok(records
        .into_iter()
        .filter_map(|r| {
            let track: Track = serde_json::from_value(r.meta).ok()?;
            (!track.id.is_empty()).then_some(Downloaded {
                track,
                bytes: r.bytes,
                saved_ms: r.saved_ms,
            })
        })
        .collect())
}

pub async fn remove_download(track_id: &str) -> ApiResult<()> {
    let (b, id) = (backend_ref()?, track_id.to_owned());
    Ok(run(async move { b.media.remove_download(&id).await }).await??)
}

pub async fn stream(track_id: &str, quality: Quality) -> ApiResult<Stream> {
    let (b, id) = (backend_ref()?, track_id.to_owned());
    let r = run(async move { b.media.resolve(&id, quality.as_str()).await }).await??;
    Ok(Stream {
        url: r.url,
        mime: r.mime,
        seconds: r.seconds,
    })
}

/// Like [`stream`], but drops any remembered address first (used for the retry after a failure,
/// when the remembered one is the thing that just failed).
pub async fn stream_fresh(track_id: &str, quality: Quality) -> ApiResult<Stream> {
    let (b, id) = (backend_ref()?, track_id.to_owned());
    let r = run(async move {
        b.media.forget(&id, quality.as_str()).await;
        b.media.resolve(&id, quality.as_str()).await
    })
    .await??;
    Ok(Stream {
        url: r.url,
        mime: r.mime,
        seconds: r.seconds,
    })
}

/// Warm the stream cache for tracks likely to play next. Fire and forget.
pub fn prefetch(track_ids: &[String], quality: Quality) {
    let Ok(b) = backend_ref() else { return };
    let _guard = b.rt.enter();
    for id in track_ids {
        b.media.prefetch(id, quality.as_str());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheInfo {
    pub bytes: u64,
    pub tracks: u64,
    pub path: PathBuf,
}

pub async fn cache_info() -> ApiResult<CacheInfo> {
    let b = backend_ref()?;
    let (bytes, tracks) = run(async move { b.media.stats().await }).await??;
    Ok(CacheInfo {
        bytes,
        tracks,
        path: b.root.join("cache"),
    })
}

pub async fn clear_cache() -> ApiResult<()> {
    let b = backend_ref()?;
    Ok(run(async move { b.media.clear().await }).await??)
}

pub fn open_cache_folder() {
    if let Ok(b) = backend_ref() {
        let _ = open::that(b.root.join("cache"));
    }
}

/// LRCLIB lookup. Makes no request and returns `Disabled` unless `enabled` (the user's opt-in).
pub async fn lyrics(enabled: bool, query: &LyricsQuery<'_>) -> ApiResult<Option<Lyrics>> {
    Ok(lyrics::fetch(enabled, query).await?)
}

// ---- Import -------------------------------------------------------------------------------

fn import_err(e: ApiError) -> ImportError {
    match e {
        ApiError::Catalog(CatalogError::Network(_)) => ImportError::Offline,
        _ => ImportError::PrivateOrEmpty,
    }
}

/// YouTube Music's browse endpoint wants list ids as `VL<id>`, share links carry the bare `<id>`.
fn browse_list_id(id: &str) -> String {
    if id.starts_with("VL") {
        id.to_owned()
    } else {
        format!("VL{id}")
    }
}

/// Read the source named by `source` without saving anything (cheap preview for the dialog).
pub async fn import_prepare(source: &Source) -> Result<PreparedImport, ImportError> {
    match source {
        Source::Spotify(id) => import_flow::prepare_spotify(import::fetch_spotify(id).await?),
        Source::Youtube(id) => {
            let c = collection(CollectionKind::Playlist, &browse_list_id(id))
                .await
                .map_err(import_err)?;
            import_flow::prepare_tracks(c.title.to_string(), c.cover, c.tracks)
        }
        Source::Shared(id) => shared_import(open_share(id).await.map_err(|e| match e {
            ShareError::Offline => ImportError::Offline,
            _ => ImportError::ShareGone,
        })?),
    }
}

/// The newest OpenWave release on GitHub (`None` when it cannot be read).
pub async fn update_latest() -> Option<String> {
    run(super::update::latest()).await.ok()?.ok()
}

/// Download and verify the installer of `version` (installed Windows build only).
pub async fn update_download(version: String) -> Option<PathBuf> {
    let dir = data_dir().join(super::update::DIR);
    run(async move { super::update::download(&version, &dir).await })
        .await
        .ok()?
        .ok()
}

/// Store a playlist (or one song, `single`) on the share relays and return its short link.
pub async fn share(name: String, tracks: Vec<Track>, single: bool) -> Result<String, ShareError> {
    run(async move { share::publish(&name, &tracks, single).await })
        .await
        .unwrap_or(Err(ShareError::Offline))
}

/// Read the list behind a share id (checked against the id, cleaned like any outside input).
pub async fn open_share(id: &str) -> Result<share::Shared, ShareError> {
    let id = id.to_owned();
    run(async move { share::fetch(&id).await })
        .await
        .unwrap_or(Err(ShareError::Offline))
}

/// A shared list as an import: it already holds catalog tracks, so nothing needs matching.
pub fn shared_import(shared: share::Shared) -> Result<PreparedImport, ImportError> {
    let cover = shared.tracks.iter().find_map(|t| t.cover.clone());
    import_flow::prepare_tracks(shared.name, cover, shared.tracks)
}

/// Match a prepared import against the catalog. `progress(done, total)` fires after each track.
/// Returns `NothingMatched` when no track could be found.
pub async fn import_run(
    prepared: PreparedImport,
    progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<ImportResult, ImportError> {
    import_flow::run(prepared, match_spotify_track, progress).await
}

async fn match_spotify_track(want: SpotifyTrack) -> Option<Track> {
    let v = query("tracks", import::search_term(&want), String::new())
        .await
        .ok()?;
    let found: Vec<Track> = convert::entries(&v, "items")
        .iter()
        .map(convert::track)
        .collect();
    import::best_match(&want, &found).map(|i| found[i].clone())
}

#[path = "api_local.rs"]
mod local;
pub use local::{
    add_local_files, import_audio, import_folder, local_lyrics, pick_cover_image,
    refresh_local_tracks,
};

/// The URL to show for a stored cover: `owcover:<hash>` (a local file's cover) becomes the
/// loopback proxy address of this launch; anything else is returned as is.
pub fn cover_src(url: &str) -> String {
    let Some(hash) = url.strip_prefix(super::local_tags::COVER_SCHEME) else {
        return url.to_owned();
    };
    match backend_ref() {
        Ok(b) if super::local_tags::valid_cover_hash(hash) => {
            format!("{}/cover-{hash}", b.media.base_url())
        }
        _ => String::new(),
    }
}

// ---- yt-dlp ---------------------------------------------------------------------------------

pub async fn ytdlp_status() -> ytdlp::Status {
    run(ytdlp::status()).await.unwrap_or_default()
}

/// Network call to github.com; only call after the user agreed.
pub async fn ytdlp_check_update() -> ApiResult<ytdlp::UpdateCheck> {
    Ok(run(ytdlp::check_update()).await??)
}

/// Download + verify + install the newest yt-dlp. Only call after the user agreed.
pub async fn ytdlp_install(
    on_progress: impl Fn(ytdlp::Progress) + Send + Sync + 'static,
) -> ApiResult<ytdlp::Installed> {
    Ok(run(ytdlp::install_latest(on_progress)).await??)
}

/// Check and install a yt-dlp executable the user picked.
pub async fn ytdlp_adopt(src: std::path::PathBuf) -> ApiResult<ytdlp::Installed> {
    Ok(run(async move { ytdlp::adopt(&src).await }).await??)
}

#[cfg(test)]
mod browse_id_tests {
    use super::browse_list_id;

    #[test]
    fn bare_youtube_list_ids_get_the_browse_prefix_once() {
        assert_eq!(browse_list_id("PLabc"), "VLPLabc");
        assert_eq!(browse_list_id("VLPLabc"), "VLPLabc");
    }
}
