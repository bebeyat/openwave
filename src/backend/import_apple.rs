//! Apple Music albums and playlists. Albums come from the public iTunes lookup API; playlists
//! from the `serialized-server-data` JSON embedded in the public music.apple.com page. The songs
//! are matched like Spotify's (title, artists, duration).
use super::{capped, http, read_capped, SpotifyPlaylist, SpotifyTrack, MAX_TRACKS};
use crate::backend::error::ImportError;
use serde_json::Value;

const ITUNES_BASE: &str = "https://itunes.apple.com";
const MUSIC_BASE: &str = "https://music.apple.com";
/// Playlist pages carry the whole track list inline and can be large.
const MAX_APPLE_PAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppleKind {
    Album,
    Playlist,
}

/// A validated `music.apple.com/<country>/<album|playlist>/<slug>/<id>` link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppleLink {
    pub kind: AppleKind,
    pub country: String,
    pub slug: String,
    pub id: String,
}

fn country(s: &str) -> Option<String> {
    (s.len() == 2 && s.bytes().all(|b| b.is_ascii_lowercase())).then(|| s.to_owned())
}

fn slug(s: &str) -> Option<String> {
    (!s.is_empty()
        && s.len() <= 200
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'%' | b'.')))
    .then(|| s.to_owned())
}

fn id_for(kind: AppleKind, s: &str) -> Option<String> {
    let ok = match kind {
        AppleKind::Album => (1..=15).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()),
        AppleKind::Playlist => {
            s.len() <= 80
                && s.strip_prefix("pl.").is_some_and(|r| {
                    !r.is_empty()
                        && r.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                })
        }
    };
    ok.then(|| s.to_owned())
}

/// The album or playlist of an Apple Music URL (a song link, `?i=`, is not one).
pub fn parse(url: &reqwest::Url) -> Option<AppleLink> {
    if url.query_pairs().any(|(k, _)| k == "i") {
        return None;
    }
    let parts: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (cc, kind, rest) = match parts.as_slice() {
        [cc, kind, rest @ ..] => (*cc, *kind, rest),
        _ => return None,
    };
    let kind = match kind {
        "album" => AppleKind::Album,
        "playlist" => AppleKind::Playlist,
        _ => return None,
    };
    let (slug, id) = match rest {
        [id] => ("x".to_owned(), *id),
        [s, id] => (slug(s)?, *id),
        _ => return None,
    };
    Some(AppleLink {
        kind,
        country: country(cc)?,
        slug,
        id: id_for(kind, id)?,
    })
}

/// Name and songs of an iTunes `lookup?entity=song` answer.
pub(crate) fn parse_lookup(v: &Value) -> SpotifyPlaylist {
    let results = v["results"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let name = results
        .iter()
        .find(|r| r["wrapperType"] == "collection")
        .and_then(|r| r["collectionName"].as_str())
        .map(capped)
        .unwrap_or_default();
    let tracks = results
        .iter()
        .filter(|r| r["wrapperType"] == "track")
        .filter_map(|r| {
            song(
                r["trackName"].as_str()?,
                &r["artistName"],
                &r["trackTimeMillis"],
            )
        })
        .take(MAX_TRACKS)
        .collect();
    SpotifyPlaylist {
        name,
        tracks,
        complete: true,
    }
}

fn song(title: &str, artists: &Value, millis: &Value) -> Option<SpotifyTrack> {
    let title = title.trim();
    (!title.is_empty()).then(|| SpotifyTrack {
        title: capped(title),
        artists: capped(artists.as_str().unwrap_or("")),
        duration_ms: millis.as_u64().unwrap_or(0),
    })
}

/// Name and songs from the `serialized-server-data` JSON of a playlist page.
pub(crate) fn parse_playlist_page(html: &str) -> Result<SpotifyPlaylist, ImportError> {
    const START: &str = "id=\"serialized-server-data\">";
    let from = html.find(START).ok_or(ImportError::Unreadable)? + START.len();
    let to = html[from..]
        .find("</script>")
        .ok_or(ImportError::Unreadable)?
        + from;
    let data: Value = serde_json::from_str(&html[from..to]).map_err(|_| ImportError::Unreadable)?;
    let sections = data["data"][0]["data"]["sections"]
        .as_array()
        .ok_or(ImportError::Unreadable)?;
    let items_of = |kind: &str| {
        sections
            .iter()
            .filter(|s| s["itemKind"] == kind)
            .filter_map(|s| s["items"].as_array())
            .flatten()
            .collect::<Vec<_>>()
    };
    let name = items_of("containerDetailHeaderLockup")
        .first()
        .and_then(|h| h["title"].as_str())
        .map(capped)
        .unwrap_or_default();
    let tracks = items_of("trackLockup")
        .into_iter()
        .filter_map(|t| song(t["title"].as_str()?, &t["artistName"], &t["duration"]))
        .take(MAX_TRACKS)
        .collect();
    Ok(SpotifyPlaylist {
        name,
        tracks,
        complete: true,
    })
}

fn status_error(status: reqwest::StatusCode) -> Option<ImportError> {
    match status.as_u16() {
        200..=299 => None,
        404 | 410 => Some(ImportError::PrivateOrEmpty),
        _ => Some(ImportError::Unreadable),
    }
}

async fn get(url: &str) -> Result<reqwest::Response, ImportError> {
    http()?
        .get(url)
        .send()
        .await
        .map_err(|_| ImportError::Offline)
}

/// A redirect to another page of the same kind on music.apple.com (a renamed slug).
fn same_site_redirect(base: &str, response: &reqwest::Response) -> Option<String> {
    let to = response.headers().get("location")?.to_str().ok()?;
    let url = reqwest::Url::parse(base).ok()?.join(to).ok()?;
    let ok = url.as_str().starts_with(&format!("{base}/"))
        && parse(&url).is_some_and(|l| l.kind == AppleKind::Playlist);
    ok.then(|| url.to_string())
}

async fn fetch_playlist(base: &str, link: &AppleLink) -> Result<SpotifyPlaylist, ImportError> {
    let url = format!("{base}/{}/playlist/{}/{}", link.country, link.slug, link.id);
    let mut response = get(&url).await?;
    if response.status().is_redirection() {
        let next = same_site_redirect(base, &response).ok_or(ImportError::Unreadable)?;
        response = get(&next).await?;
    }
    if let Some(e) = status_error(response.status()) {
        return Err(e);
    }
    parse_playlist_page(&read_capped(response, MAX_APPLE_PAGE_BYTES).await?)
}

async fn fetch_album(base: &str, link: &AppleLink) -> Result<SpotifyPlaylist, ImportError> {
    let url = format!(
        "{base}/lookup?id={}&entity=song&country={}",
        link.id, link.country
    );
    let response = get(&url).await?;
    if let Some(e) = status_error(response.status()) {
        return Err(e);
    }
    let body = read_capped(response, super::MAX_PAGE_BYTES).await?;
    let v: Value = serde_json::from_str(&body).map_err(|_| ImportError::Unreadable)?;
    Ok(parse_lookup(&v))
}

/// Read an Apple Music album or playlist.
pub async fn fetch(link: &AppleLink) -> Result<SpotifyPlaylist, ImportError> {
    fetch_from(ITUNES_BASE, MUSIC_BASE, link).await
}

pub(crate) async fn fetch_from(
    itunes: &str,
    music: &str,
    link: &AppleLink,
) -> Result<SpotifyPlaylist, ImportError> {
    let list = match link.kind {
        AppleKind::Album => fetch_album(itunes, link).await?,
        AppleKind::Playlist => fetch_playlist(music, link).await?,
    };
    if list.tracks.is_empty() {
        return Err(ImportError::PrivateOrEmpty);
    }
    Ok(list)
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("import_apple_tests.rs");
}
