//! Reading the signed-in user's Spotify library (Web API): their playlists, the songs of one
//! playlist and Liked Songs. The songs go into the same matching as a pasted Spotify link.
use super::error::ImportError;
use super::import::{capped, SpotifyPlaylist, SpotifyTrack, MAX_TRACKS};
use serde_json::Value;
use std::time::Duration;

pub const API: &str = "https://api.spotify.com";
/// The playlist picker lists at most this many.
const MAX_PLAYLISTS: usize = 200;

/// One of the user's playlists, for the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserPlaylist {
    pub id: String,
    pub name: String,
    pub count: usize,
}

/// What to import from the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Liked,
    Playlist(String),
}

fn http() -> Result<reqwest::Client, ImportError> {
    reqwest::Client::builder()
        .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ImportError::Unreadable)
}

async fn get_json(client: &reqwest::Client, url: &str, token: &str) -> Result<Value, ImportError> {
    let res = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| ImportError::Offline)?;
    match res.status().as_u16() {
        200..=299 => res.json().await.map_err(|_| ImportError::Unreadable),
        401 | 403 | 404 => Err(ImportError::PrivateOrEmpty),
        _ => Err(ImportError::Unreadable),
    }
}

/// Entries on one page (including ones that are skipped, such as local files).
fn page_len(v: &Value) -> usize {
    v["items"].as_array().map_or(0, Vec::len)
}

/// Songs of one page of `/me/tracks` or `/playlists/{id}/tracks` (the song is under `track`, or
/// `item` in the newer `/items` answer), and the total.
pub(crate) fn parse_tracks(v: &Value) -> (Vec<SpotifyTrack>, usize) {
    let tracks = v["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|it| {
            let t = if it["track"].is_object() {
                &it["track"]
            } else {
                &it["item"]
            };
            let title = t["name"].as_str()?.trim();
            let artists: Vec<&str> = t["artists"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect())
                .unwrap_or_default();
            (!title.is_empty()).then(|| SpotifyTrack {
                title: capped(title),
                artists: capped(&artists.join(", ")),
                duration_ms: t["duration_ms"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    let total = v["total"].as_u64().map_or(0, |n| n as usize);
    (tracks, total)
}

pub(crate) fn parse_playlists(v: &Value) -> Vec<UserPlaylist> {
    v["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| {
            let id = p["id"]
                .as_str()
                .filter(|id| super::import::spotify_id(id).is_some())?;
            let tracks = if p["tracks"].is_object() {
                &p["tracks"]
            } else {
                &p["items"]
            };
            Some(UserPlaylist {
                id: id.to_owned(),
                name: capped(p["name"].as_str().unwrap_or("")),
                count: tracks["total"].as_u64().map_or(0, |n| n as usize),
            })
        })
        .collect()
}

/// The user's playlists (own and followed), first [`MAX_PLAYLISTS`].
pub async fn playlists(api: &str, token: &str) -> Result<Vec<UserPlaylist>, ImportError> {
    let client = http()?;
    let (mut out, mut offset) = (vec![], 0);
    while offset < MAX_PLAYLISTS {
        let url = format!("{api}/v1/me/playlists?limit=50&offset={offset}");
        let v = get_json(&client, &url, token).await?;
        if page_len(&v) == 0 {
            break;
        }
        offset += page_len(&v);
        out.extend(parse_playlists(&v));
    }
    out.truncate(MAX_PLAYLISTS);
    Ok(out)
}

/// The songs at `path`, page by page up to [`MAX_TRACKS`], and whether that was all of them.
async fn read_pages(
    client: &reqwest::Client,
    url_base: &str,
    token: &str,
    page: usize,
) -> Result<(Vec<SpotifyTrack>, bool), ImportError> {
    let mut list: Vec<SpotifyTrack> = vec![];
    let (mut offset, mut total) = (0, 0);
    while list.len() < MAX_TRACKS {
        let url = format!("{url_base}?limit={page}&offset={offset}");
        let v = get_json(client, &url, token).await?;
        let (got, all) = parse_tracks(&v);
        total = all;
        if page_len(&v) == 0 {
            break;
        }
        offset += page_len(&v);
        list.extend(got);
        if offset >= all {
            break;
        }
    }
    let complete = offset >= total && list.len() <= MAX_TRACKS;
    list.truncate(MAX_TRACKS);
    Ok((list, complete))
}

/// The songs of `pick`, page by page up to [`MAX_TRACKS`]; `name` names the list.
pub async fn tracks(
    api: &str,
    token: &str,
    pick: &Pick,
    name: String,
) -> Result<SpotifyPlaylist, ImportError> {
    let client = http()?;
    let read = match pick {
        Pick::Liked => read_pages(&client, &format!("{api}/v1/me/tracks"), token, 50).await,
        Pick::Playlist(id) => {
            let id = super::import::spotify_id(id).ok_or(ImportError::BadLink)?;
            let base = format!("{api}/v1/playlists/{id}");
            // Newer API versions moved the songs from `/tracks` to `/items`.
            match read_pages(&client, &format!("{base}/tracks"), token, 100).await {
                Err(ImportError::PrivateOrEmpty) => {
                    read_pages(&client, &format!("{base}/items"), token, 100).await
                }
                other => other,
            }
        }
    };
    let (tracks, complete) = read?;
    if tracks.is_empty() {
        return Err(ImportError::PrivateOrEmpty);
    }
    Ok(SpotifyPlaylist {
        name,
        tracks,
        complete,
    })
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("spotify_account_tests.rs");
}
