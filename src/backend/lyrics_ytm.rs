//! Time-synced lyrics from YouTube Music. They are timed against the very recording the app
//! streams, so they line up better than community LRC files. Two InnerTube calls: `next` (web
//! client) names the song's lyrics page, `browse` with the Android Music client returns the lines
//! with their cue times. Sends only the video id, which YouTube already gets for playback.

use std::{sync::OnceLock, time::Duration};

use serde_json::{json, Value};

use super::error::LyricsError;
use crate::model::{LyricLine, Lyrics};

pub const YTM_BASE: &str = "https://music.youtube.com";
const PROVIDER: &str = "YouTube Music";

fn http() -> Result<&'static reqwest::Client, LyricsError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| LyricsError::Network(e.clone()))
}

fn web_client() -> Value {
    json!({ "clientName": "WEB_REMIX", "clientVersion": "1.20241023.01.00", "hl": "en" })
}

fn android_client() -> Value {
    json!({ "clientName": "ANDROID_MUSIC", "clientVersion": "7.21.50", "androidSdkVersion": 30, "hl": "en" })
}

async fn post(
    base: &str,
    endpoint: &str,
    body: Value,
    client: Value,
) -> Result<Value, LyricsError> {
    let mut body = body;
    body["context"] = json!({ "client": client });
    let res = http()?
        .post(format!("{base}/youtubei/v1/{endpoint}?prettyPrint=false"))
        .header("Origin", YTM_BASE)
        .json(&body)
        .send()
        .await
        .map_err(|e| LyricsError::Network(e.to_string()))?;
    if !res.status().is_success() {
        return Err(LyricsError::Unavailable);
    }
    res.json().await.map_err(|_| LyricsError::Unavailable)
}

/// First value stored under `key` anywhere in `v` (depth-first).
fn find<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|x| find(x, key))),
        Value::Array(list) => list.iter().find_map(|x| find(x, key)),
        _ => None,
    }
}

/// The `MPLYt…` browse id of the lyrics tab in a `next` answer.
pub(crate) fn lyrics_browse_id(next: &Value) -> Option<String> {
    let tabs = find(next, "watchNextTabbedResultsRenderer")?["tabs"].as_array()?;
    tabs.iter()
        .filter_map(|t| t["tabRenderer"]["endpoint"]["browseEndpoint"]["browseId"].as_str())
        .find(|id| id.starts_with("MPLYt"))
        .map(str::to_owned)
}

fn ms(v: &Value) -> Option<u64> {
    v.as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| v.as_u64())
}

/// Lines of a `browse` answer: synced when every line has a cue time, plain otherwise.
pub(crate) fn parse_browse(browse: &Value) -> Option<Lyrics> {
    let data = find(browse, "timedLyricsData")?.as_array()?;
    let lines: Vec<(Option<u64>, String)> = data
        .iter()
        .filter_map(|l| {
            let text = l["lyricLine"].as_str()?.trim().replace('♪', "");
            Some((
                ms(&l["cueRange"]["startTimeMilliseconds"]),
                text.trim().to_owned(),
            ))
        })
        .collect();
    if lines.iter().all(|(_, t)| t.is_empty()) {
        return None;
    }
    let plain = lines
        .iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let synced = if lines.iter().all(|(start, _)| start.is_some()) {
        lines
            .into_iter()
            .map(|(start, text)| LyricLine {
                start_ms: start.unwrap_or(0),
                text,
                words: vec![],
            })
            .collect()
    } else {
        vec![]
    };
    Some(Lyrics {
        plain,
        synced,
        provider: PROVIDER.into(),
    })
}

/// Lyrics YouTube Music has for `video_id`, if any.
pub async fn fetch(video_id: &str) -> Result<Option<Lyrics>, LyricsError> {
    fetch_from(YTM_BASE, video_id).await
}

pub(crate) async fn fetch_from(base: &str, video_id: &str) -> Result<Option<Lyrics>, LyricsError> {
    let next = post(base, "next", json!({ "videoId": video_id }), web_client()).await?;
    let Some(browse_id) = lyrics_browse_id(&next) else {
        return Ok(None);
    };
    let browse = post(
        base,
        "browse",
        json!({ "browseId": browse_id }),
        android_client(),
    )
    .await?;
    Ok(parse_browse(&browse))
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("lyrics_ytm_tests.rs");
}
