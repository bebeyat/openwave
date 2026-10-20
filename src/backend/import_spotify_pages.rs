//! Spotify lists longer than the embed page: the rest is read from the web player's GraphQL
//! endpoint (`pathfinder`) with the anonymous token the embed page carries. The public Web API
//! (`api.spotify.com/v1`) answers that token with 429 "QUOTA_EXCEEDED", so it is not used.
use super::{capped, SpotifyPlaylist, SpotifyTrack, MAX_TRACKS};
use serde_json::{json, Value};

pub(super) const PATHFINDER: &str = "https://api-partner.spotify.com";
/// Persisted query id of `fetchPlaylist` in Spotify's web player. When Spotify retires it the
/// request fails and the import keeps the first 100 tracks with the usual warning.
const FETCH_PLAYLIST_HASH: &str =
    "19ff1327c29e99c208c86d7a9d8f1929cfdf3d3202a0ff4253c821f1901aa94d";
const PAGE: usize = 100;

/// The anonymous access token of the embed page's `__NEXT_DATA__` (token characters only).
pub(super) fn embed_token(html: &str) -> Option<String> {
    const KEY: &str = "\"accessToken\":\"";
    let from = html.find(KEY)? + KEY.len();
    let token = &html[from..from + html[from..].find('"')?];
    let ok = (20..=512).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    ok.then(|| token.to_owned())
}

/// Tracks of one `fetchPlaylist` page and the list's total length.
pub(super) fn parse_page(v: &Value) -> Option<(Vec<SpotifyTrack>, usize)> {
    let content = &v["data"]["playlistV2"]["content"];
    let total = usize::try_from(content["totalCount"].as_u64()?).ok()?;
    let tracks = content["items"]
        .as_array()?
        .iter()
        .filter_map(|item| {
            let t = &item["itemV2"]["data"];
            let title = t["name"].as_str()?.trim();
            let artists: Vec<&str> = t["artists"]["items"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x["profile"]["name"].as_str())
                        .collect()
                })
                .unwrap_or_default();
            (!title.is_empty()).then(|| SpotifyTrack {
                title: capped(title),
                artists: capped(&artists.join(", ")),
                duration_ms: t["trackDuration"]["totalMilliseconds"]
                    .as_u64()
                    .unwrap_or(0),
            })
        })
        .collect();
    Some((tracks, total))
}

async fn fetch_page(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    id: &str,
    offset: usize,
) -> Option<(Vec<SpotifyTrack>, usize)> {
    let variables =
        json!({ "uri": format!("spotify:playlist:{id}"), "offset": offset, "limit": PAGE });
    let extensions =
        json!({ "persistedQuery": { "version": 1, "sha256Hash": FETCH_PLAYLIST_HASH } });
    let response = client
        .get(format!("{base}/pathfinder/v1/query"))
        .bearer_auth(token)
        .query(&[
            ("operationName", "fetchPlaylist".to_owned()),
            ("variables", variables.to_string()),
            ("extensions", extensions.to_string()),
        ])
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = super::read_capped(response, super::MAX_PAGE_BYTES)
        .await
        .ok()?;
    parse_page(&serde_json::from_str(&body).ok()?)
}

/// `list` (the embed page's first tracks) extended page by page up to [`MAX_TRACKS`]. A failed
/// page ends the reading; the list is then marked incomplete.
pub(super) async fn read_on(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    id: &str,
    list: SpotifyPlaylist,
) -> SpotifyPlaylist {
    let mut tracks = list.tracks;
    let mut total = None;
    while tracks.len() < MAX_TRACKS {
        let Some((page, all)) = fetch_page(client, base, token, id, tracks.len()).await else {
            break;
        };
        total = Some(all);
        if page.is_empty() {
            break;
        }
        tracks.extend(page);
        if tracks.len() >= all {
            break;
        }
    }
    tracks.truncate(MAX_TRACKS);
    let complete = total.is_some_and(|all| tracks.len() >= all);
    SpotifyPlaylist {
        tracks,
        complete,
        ..list
    }
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("import_spotify_pages_tests.rs");
}
