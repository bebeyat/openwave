//! Playlist import: link parsing, the Spotify public embed page reader and track matching.
//! Spotify links are read from the public embed page (no account); matching against YouTube Music
//! happens in `import_flow.rs`. Everything here is pure except [`fetch_spotify`].
use super::error::ImportError;
use crate::model::Track;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

#[path = "import_spotify_pages.rs"]
mod spotify_pages;

const SPOTIFY_EMBED_BASE: &str = "https://open.spotify.com";
/// Spotify's embed page lists at most this many tracks.
pub const SPOTIFY_EMBED_LIMIT: usize = 100;
/// The embed page is read through this many bytes at most (it is normally a few hundred KB).
pub(crate) const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
/// Parsing keeps at most this many tracks and this many characters per title, artist list and name.
pub(crate) const MAX_TRACKS: usize = 500;
pub(crate) const MAX_TEXT_CHARS: usize = 200;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Spotify(String),
    Youtube(String),
    /// An OpenWave share link (`share::BASE_URL`); holds the share id.
    Shared(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpotifyTrack {
    pub title: String,
    pub artists: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpotifyPlaylist {
    pub name: String,
    pub tracks: Vec<SpotifyTrack>,
    /// Every track of the list was read (false: cut at the embed limit or [`MAX_TRACKS`]).
    pub complete: bool,
}

fn spotify_id(s: &str) -> Option<String> {
    (s.len() == 22 && s.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| s.to_owned())
}

fn youtube_list_id(s: &str) -> Option<String> {
    (!s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then(|| s.to_owned())
}

fn parse_url(input: &str) -> Option<reqwest::Url> {
    reqwest::Url::parse(input)
        .or_else(|_| reqwest::Url::parse(&format!("https://{input}")))
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}

fn host_of(url: &reqwest::Url) -> String {
    url.host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

/// Recognise a Spotify or YouTube playlist link (or `spotify:playlist:<id>` URI).
pub fn parse_source(input: &str) -> Option<Source> {
    let input = input.trim();
    if input.contains("openwave") {
        return super::share::parse_link(input).map(Source::Shared);
    }
    if let Some(id) = input.strip_prefix("spotify:playlist:") {
        return spotify_id(id).map(Source::Spotify);
    }
    let url = parse_url(input)?;
    let host = host_of(&url);
    if host == "open.spotify.com" {
        // Paths look like /playlist/<id>, /intl-tr/playlist/<id> or /embed/playlist/<id>.
        let mut parts = url.path_segments()?;
        while let Some(part) = parts.next() {
            if part == "playlist" {
                return spotify_id(parts.next()?).map(Source::Spotify);
            }
        }
        return None;
    }
    if matches!(
        host.as_str(),
        "youtube.com" | "music.youtube.com" | "m.youtube.com"
    ) {
        let id = url
            .query_pairs()
            .find(|(k, _)| k == "list")
            .map(|(_, v)| v.into_owned())?;
        return youtube_list_id(&id).map(Source::Youtube);
    }
    None
}

/// Like [`parse_source`] but says why a link was rejected: a valid Spotify/YouTube link that points
/// at a single track, album, artist or video is `NotAPlaylist`; anything else is `BadLink`.
pub fn parse_link(input: &str) -> Result<Source, ImportError> {
    if let Some(source) = parse_source(input) {
        return Ok(source);
    }
    let input = input.trim();
    if let Some(kind) = input
        .strip_prefix("spotify:")
        .and_then(|r| r.split(':').next())
    {
        let single = ["track", "album", "artist", "episode", "show"];
        return Err(if single.contains(&kind) {
            ImportError::NotAPlaylist
        } else {
            ImportError::BadLink
        });
    }
    let Some(url) = parse_url(input) else {
        return Err(ImportError::BadLink);
    };
    let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    let first = segments.first().copied().unwrap_or("");
    let is_single = match host_of(&url).as_str() {
        "open.spotify.com" => segments
            .iter()
            .any(|s| ["track", "album", "artist", "episode", "show"].contains(s)),
        "youtu.be" => !first.is_empty(),
        "youtube.com" | "music.youtube.com" | "m.youtube.com" => {
            url.query_pairs().any(|(k, _)| k == "v")
                || ["watch", "shorts", "channel", "browse"].contains(&first)
        }
        _ => false,
    };
    Err(if is_single {
        ImportError::NotAPlaylist
    } else {
        ImportError::BadLink
    })
}

/// Trimmed and cut to [`MAX_TEXT_CHARS`] characters.
fn capped(s: &str) -> String {
    s.trim().chars().take(MAX_TEXT_CHARS).collect()
}

/// Extract the playlist from the `__NEXT_DATA__` JSON of Spotify's embed page.
pub fn parse_spotify_embed(html: &str) -> Result<SpotifyPlaylist, ImportError> {
    const START: &str = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let from = html.find(START).ok_or(ImportError::Unreadable)? + START.len();
    let to = html[from..]
        .find("</script>")
        .ok_or(ImportError::Unreadable)?
        + from;
    let data: Value = serde_json::from_str(&html[from..to]).map_err(|_| ImportError::Unreadable)?;
    let entity = &data["props"]["pageProps"]["state"]["data"]["entity"];
    let list = entity["trackList"]
        .as_array()
        .ok_or(ImportError::PrivateOrEmpty)?;
    let tracks: Vec<SpotifyTrack> = list
        .iter()
        .filter_map(|t| {
            let title = t["title"].as_str()?.trim();
            (!title.is_empty()).then(|| SpotifyTrack {
                title: capped(title),
                artists: capped(&t["subtitle"].as_str().unwrap_or("").replace('\u{a0}', " ")),
                duration_ms: t["duration"].as_u64().unwrap_or(0),
            })
        })
        .take(MAX_TRACKS)
        .collect();
    let name = capped(
        entity["name"]
            .as_str()
            .or(entity["title"].as_str())
            .unwrap_or(""),
    );
    let complete = tracks.len() < SPOTIFY_EMBED_LIMIT;
    Ok(SpotifyPlaylist {
        name,
        tracks,
        complete,
    })
}

fn http() -> Result<&'static reqwest::Client, ImportError> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36")
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok()
        })
        .as_ref()
        .ok_or(ImportError::Unreadable)
}

/// Only "not found" means the playlist is private or gone; a rate limit, redirect or server error
/// is temporary ("try again in a moment").
fn spotify_status_error(status: reqwest::StatusCode) -> Option<ImportError> {
    match status.as_u16() {
        200..=299 => None,
        401 | 403 | 404 | 410 => Some(ImportError::PrivateOrEmpty),
        _ => Some(ImportError::Unreadable),
    }
}

/// Download and parse a Spotify playlist (fixed host, validated 22-character id). Lists longer
/// than the embed page are read on through Spotify's web player API with the embed page's
/// anonymous token; if that fails the first 100 tracks are kept.
pub async fn fetch_spotify(id: &str) -> Result<SpotifyPlaylist, ImportError> {
    fetch_spotify_from(SPOTIFY_EMBED_BASE, spotify_pages::PATHFINDER, id).await
}

pub(crate) async fn fetch_spotify_from(
    embed_base: &str,
    pathfinder: &str,
    id: &str,
) -> Result<SpotifyPlaylist, ImportError> {
    let id = spotify_id(id).ok_or(ImportError::BadLink)?;
    let response = http()?
        .get(format!("{embed_base}/embed/playlist/{id}"))
        .send()
        .await
        .map_err(|_| ImportError::Offline)?;
    if let Some(e) = spotify_status_error(response.status()) {
        return Err(e);
    }
    let html = read_capped(response, MAX_PAGE_BYTES).await?;
    let list = parse_spotify_embed(&html)?;
    if list.tracks.is_empty() {
        return Err(ImportError::PrivateOrEmpty);
    }
    if list.complete {
        return Ok(list);
    }
    let Some(token) = spotify_pages::embed_token(&html) else {
        return Ok(list);
    };
    Ok(spotify_pages::read_on(http()?, pathfinder, &token, &id, list).await)
}

/// Read the body as text, giving up with [`ImportError::Unreadable`] past `cap` bytes.
pub(crate) async fn read_capped(
    mut response: reqwest::Response,
    cap: usize,
) -> Result<String, ImportError> {
    let too_big = |len: u64| usize::try_from(len).map_or(true, |l| l > cap);
    if response.content_length().is_some_and(too_big) {
        return Err(ImportError::Unreadable);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ImportError::Offline)? {
        if body.len() + chunk.len() > cap {
            return Err(ImportError::Unreadable);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Folded (see `util::fold`) alphanumerics only, with "(feat. …)", "[…]" and "- remastered"-style
/// suffixes removed.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in crate::util::fold(s).chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            c if c.is_alphanumeric() => out.push(c),
            c if c.is_whitespace() => out.push(' '),
            _ => {}
        }
    }
    let cut = out
        .find(" remaster")
        .or_else(|| out.find(" feat"))
        .unwrap_or(out.len());
    out[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
}

fn score(want: &SpotifyTrack, cand: &Track) -> i32 {
    let (wt, ct) = (normalize(&want.title), normalize(&cand.title));
    let mut s = 0;
    if !wt.is_empty() && wt == ct {
        s += 50;
    } else if !wt.is_empty() && !ct.is_empty() && (ct.contains(&wt) || wt.contains(&ct)) {
        s += 25;
    }
    let want_artists = normalize(&want.artists);
    let cand_artists = normalize(&cand.artists_label());
    let overlap = want_artists
        .split_whitespace()
        .filter(|w| w.len() > 1 && cand_artists.split_whitespace().any(|c| c == *w))
        .count();
    s += (overlap as i32 * 10).min(30);
    if want.duration_ms > 0 && cand.duration_ms > 0 {
        s += match want.duration_ms.abs_diff(cand.duration_ms) {
            0..=3_000 => 20,
            3_001..=8_000 => 8,
            8_001..=20_000 => 0,
            _ => -25,
        };
    }
    s
}

/// Index of the best YouTube Music candidate for `want`, if any is convincing enough.
pub fn best_match(want: &SpotifyTrack, candidates: &[Track]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, score(want, c)))
        .filter(|(_, s)| *s >= 40)
        .max_by_key(|(i, s)| (*s, std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// Search term used to look a Spotify track up on YouTube Music.
pub fn search_term(want: &SpotifyTrack) -> String {
    let first_artist = want.artists.split(',').next().unwrap_or("").trim();
    format!("{} {}", want.title, first_artist).trim().to_owned()
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use reqwest::StatusCode as S;

    #[test]
    fn only_not_found_reads_as_private_and_rate_limits_ask_to_retry() {
        assert_eq!(spotify_status_error(S::OK), None);
        assert_eq!(
            spotify_status_error(S::NOT_FOUND),
            Some(ImportError::PrivateOrEmpty)
        );
        for temporary in [S::TOO_MANY_REQUESTS, S::FOUND, S::SERVICE_UNAVAILABLE] {
            assert_eq!(
                spotify_status_error(temporary),
                Some(ImportError::Unreadable)
            );
        }
    }
}
