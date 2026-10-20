use super::{error::CatalogError, media::locked};
use parse::{collect_albums, collect_tracks, image};
use search::collect_mrlir;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashSet, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;
use ytmapi_rs::common::{
    AlbumID, ArtistChannelID, MoodCategoryParams, PlaylistID, VideoID, YoutubeID,
};
use ytmapi_rs::query::search::{
    AlbumsFilter, ArtistsFilter, PlaylistsFilter, SearchQuery, SongsFilter,
};
use ytmapi_rs::query::{GetMoodPlaylistsQuery, GetPlaylistTracksQuery};
use ytmapi_rs::{auth::noauth::NoAuthToken, YtMusic};

#[path = "catalog_discover.rs"]
pub mod discover;
#[path = "catalog_fallback.rs"]
mod fallback;
#[path = "catalog_moods.rs"]
mod moods;
#[path = "catalog_parse.rs"]
mod parse;
#[path = "catalog_search.rs"]
mod search;

const CACHE_TTL: Duration = Duration::from_secs(300);
const CACHE_ENTRIES: usize = 40;
const MAX_TERM_CHARS: usize = 180;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub key: String,
    pub kind: String,
    pub name: String,
    pub performer: String,
    pub seconds: u64,
    pub artwork: String,
    pub album: String,
    pub artist_key: String,
    pub album_key: String,
}

/// Transport failures (offline, DNS, TLS, timeout) are `Network`; anything else the server or
/// parser reports is `BadResponse`, so private/missing items are not mistaken for being offline.
fn net<E: std::fmt::Display>(e: E) -> CatalogError {
    let text = e.to_string();
    let lower = text.to_lowercase();
    let transport = [
        "error sending request",
        "dns",
        "connect",
        "timed out",
        "timeout",
        "tls",
        "network",
    ];
    if transport.iter().any(|k| lower.contains(k)) {
        CatalogError::Network(text)
    } else {
        CatalogError::BadResponse(text)
    }
}
fn bad<E: std::fmt::Display>(e: E) -> CatalogError {
    CatalogError::BadResponse(e.to_string())
}

pub struct Catalog {
    api: OnceCell<YtMusic<NoAuthToken>>,
    recent: Mutex<VecDeque<(String, Instant, Value)>>,
}
impl Catalog {
    pub fn new() -> Self {
        Self {
            api: OnceCell::new(),
            recent: Mutex::new(VecDeque::new()),
        }
    }
    /// Create the YouTube Music client ahead of the first search so it does not pay for it.
    pub async fn warm(&self) {
        let _ = self
            .api
            .get_or_try_init(|| async { YtMusic::new_unauthenticated().await.map_err(net) })
            .await;
    }

    pub async fn query(&self, section: &str, term: &str, key: &str) -> Result<Value, CatalogError> {
        validate(term, key)?;
        let cache_key = format!("{section}/{term}/{key}");
        if let Some(v) = self.cached(&cache_key) {
            return Ok(v);
        }
        if section == "tracks" {
            let value = self.search_tracks(term).await?;
            self.remember(cache_key, &value);
            return Ok(value);
        }
        let api = self.api().await?;
        let value = match section {
            "artists" | "albums" | "lists" => search_section(api, section, term).await?,
            "genres" => moods::genre_items(&api.get_mood_categories().await.map_err(net)?),
            "genre" => genre_section(api, key).await?,
            _ => browse_section(api, section, key).await?,
        };
        self.remember(cache_key, &value);
        Ok(value)
    }

    async fn api(&self) -> Result<&YtMusic<NoAuthToken>, CatalogError> {
        self.api
            .get_or_try_init(|| async { YtMusic::new_unauthenticated().await.map_err(net) })
            .await
    }

    /// Songs from YouTube Music, or from yt-dlp when it fails (or is being skipped after
    /// repeated failures).
    async fn search_tracks(&self, term: &str) -> Result<Value, CatalogError> {
        if fallback::skipping() {
            return match fallback::ytdlp_tracks(term).await {
                Ok(v) => Ok(v),
                Err(_) => search_section(self.api().await?, "tracks", term).await,
            };
        }
        let ytm = match self.api().await {
            Ok(api) => search_section(api, "tracks", term).await,
            Err(e) => Err(e),
        };
        fallback::record(ytm.is_ok());
        match ytm {
            Ok(v) => Ok(v),
            Err(e) => fallback::ytdlp_tracks(term).await.map_err(|_| e),
        }
    }

    fn cached(&self, cache_key: &str) -> Option<Value> {
        locked(&self.recent)
            .iter()
            .find(|(k, t, _)| k == cache_key && t.elapsed() < CACHE_TTL)
            .map(|(_, _, v)| v.clone())
    }

    fn remember(&self, cache_key: String, value: &Value) {
        let mut cache = locked(&self.recent);
        if cache.len() >= CACHE_ENTRIES {
            cache.pop_front();
        }
        cache.push_back((cache_key, Instant::now(), value.clone()));
    }
}

/// Terms and ids are checked before any request is made.
fn validate(term: &str, key: &str) -> Result<(), CatalogError> {
    let bad_key = !key.is_empty()
        && !key
            .bytes()
            // `%` and `=` occur in genre browse params (base64, sometimes percent-encoded).
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'%' | b'='));
    if term.chars().count() > MAX_TERM_CHARS || key.len() > MAX_TERM_CHARS || bad_key {
        Err(CatalogError::InvalidRequest)
    } else {
        Ok(())
    }
}

/// `tracks`, `artists`, `albums` or `lists`: one filtered search, rows read from the raw response.
async fn search_section(
    api: &YtMusic<NoAuthToken>,
    section: &str,
    term: &str,
) -> Result<Value, CatalogError> {
    let (raw, kind) = match section {
        "artists" => (
            api.raw_json_query(SearchQuery::new_filtered(term, ArtistsFilter))
                .await,
            "artist",
        ),
        "albums" => (
            api.raw_json_query(SearchQuery::new_filtered(term, AlbumsFilter))
                .await,
            "album",
        ),
        "lists" => (
            api.raw_json_query(SearchQuery::new_filtered(term, PlaylistsFilter))
                .await,
            "list",
        ),
        _ => (
            api.raw_json_query(SearchQuery::new_filtered(term, SongsFilter))
                .await,
            "track",
        ),
    };
    let raw: Value = serde_json::from_str(&raw.map_err(net)?).map_err(bad)?;
    let mut items = vec![];
    collect_mrlir(&raw, kind, &mut items);
    Ok(json!({ "items": items }))
}

/// `genre`: the playlists of one genre (`key` is the genre's browse params).
async fn genre_section(api: &YtMusic<NoAuthToken>, key: &str) -> Result<Value, CatalogError> {
    let raw = api
        .raw_json_query(GetMoodPlaylistsQuery::new(MoodCategoryParams::from_raw(
            key,
        )))
        .await
        .map_err(net)?;
    Ok(moods::genre_playlists(
        &serde_json::from_str(&raw).map_err(bad)?,
    ))
}

/// `artist`, `album`, `list` or `radio`: one page, flattened to tracks (and an artist's releases).
async fn browse_section(
    api: &YtMusic<NoAuthToken>,
    section: &str,
    key: &str,
) -> Result<Value, CatalogError> {
    let raw = match fetch_page(api, section, key).await {
        Err(CatalogError::BadResponse(_)) if section == "list" => {
            return lenient_list(api, key).await
        }
        page => page?,
    };
    let mut tracks = vec![];
    collect_tracks(&raw, &raw, &mut tracks);
    let mut seen = HashSet::new();
    tracks.retain(|t| seen.insert(t.key.clone()));
    let mut releases = vec![];
    if section == "artist" {
        collect_albums(&raw, &mut releases);
    }
    Ok(json!({
        "items": tracks,
        "releases": releases,
        "name": raw["title"].as_str().or(raw["name"].as_str()).unwrap_or(""),
        "description": raw["description"].as_str().unwrap_or(""),
        "artwork": image(&raw),
    }))
}

/// The typed playlist reader rejects a whole list when one row lacks a field (a song whose
/// artist has no page, common in video charts). Read the rows ourselves and skip only those.
async fn lenient_list(api: &YtMusic<NoAuthToken>, key: &str) -> Result<Value, CatalogError> {
    let raw = api
        .raw_json_query(GetPlaylistTracksQuery::new(PlaylistID::from_raw(key)))
        .await
        .map_err(net)?;
    let raw: Value = serde_json::from_str(&raw).map_err(bad)?;
    let mut items = vec![];
    collect_mrlir(&raw, "track", &mut items);
    let mut seen = HashSet::new();
    items.retain(|t| seen.insert(t.key.clone()));
    if items.is_empty() {
        return Err(CatalogError::BadResponse("empty list".into()));
    }
    let header = search::find_header(&raw);
    Ok(json!({
        "items": items,
        "releases": [],
        "name": header.and_then(|h| h.pointer("/title/runs/0/text")).and_then(Value::as_str).unwrap_or(""),
        "description": "",
        "artwork": header
            .and_then(|h| h.pointer("/thumbnail/musicThumbnailRenderer/thumbnail"))
            .map(image)
            .unwrap_or_default(),
    }))
}

async fn fetch_page(
    api: &YtMusic<NoAuthToken>,
    section: &str,
    key: &str,
) -> Result<Value, CatalogError> {
    let value = match section {
        "artist" => serde_json::to_value(
            api.get_artist(ArtistChannelID::from_raw(key))
                .await
                .map_err(net)?,
        ),
        "album" => serde_json::to_value(api.get_album(AlbumID::from_raw(key)).await.map_err(net)?),
        "list" => {
            let header = serde_json::to_value(
                api.get_playlist_details(PlaylistID::from_raw(key))
                    .await
                    .map_err(net)?,
            )
            .map_err(bad)?;
            let tracks = serde_json::to_value(
                api.get_playlist_tracks(PlaylistID::from_raw(key))
                    .await
                    .map_err(net)?,
            )
            .map_err(bad)?;
            Ok(json!({
                "title": header["title"],
                "thumbnails": header["thumbnails"],
                "description": header["description"],
                "tracks": tracks,
            }))
        }
        "radio" => serde_json::to_value(
            api.get_watch_playlist_from_video_id(VideoID::from_raw(key))
                .await
                .map_err(net)?,
        ),
        _ => return Err(CatalogError::UnknownSection),
    };
    value.map_err(bad)
}
