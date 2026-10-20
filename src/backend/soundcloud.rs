//! SoundCloud search through yt-dlp (`scsearch`), and finding the SoundCloud copy of a song that
//! YouTube cannot play. Track ids are `sc-<digits>` (see `media::soundcloud_id`).
use super::catalog::Entry;
use super::error::{CatalogError, MediaError};
use super::media::soundcloud_id;
use super::palette_net::allowed_image_url;
use super::ytdlp;
use serde_json::{json, Value};
use std::time::Duration;

const RESULTS: u32 = 20;
const SEARCH_LIMIT: Duration = Duration::from_secs(25);
/// A fallback match may differ from the wanted song by at most this much.
const MATCH_SECONDS: f64 = 5.0;

/// The 300 px artwork when offered, else the largest one, on an allowed image host.
fn artwork(e: &Value) -> String {
    let thumbs = e["thumbnails"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    thumbs
        .iter()
        .find(|t| t["id"] == "t300x300")
        .or_else(|| {
            thumbs
                .iter()
                .max_by_key(|t| t["width"].as_u64().unwrap_or(0))
        })
        .and_then(|t| t["url"].as_str())
        .filter(|u| allowed_image_url(u))
        .unwrap_or("")
        .to_owned()
}

/// Without an account SoundCloud plays only a 30-second preview of many label songs.
const PREVIEW_SECONDS: u64 = 30;

/// Song entries of a `yt-dlp --flat-playlist -J scsearchN:` answer (previews left out).
pub(crate) fn entries(v: &Value) -> Vec<Entry> {
    let Some(list) = v["entries"].as_array() else {
        return vec![];
    };
    list.iter()
        .filter_map(|e| {
            let key = format!("sc-{}", e["id"].as_str()?);
            soundcloud_id(&key)?;
            Some(Entry {
                key,
                kind: "track".into(),
                name: e["title"].as_str()?.trim().to_owned(),
                performer: e["uploader"].as_str().unwrap_or("").to_owned(),
                seconds: e["duration"]
                    .as_f64()
                    .map_or(0, |d| d.max(0.0).round() as u64),
                artwork: artwork(e),
                ..Entry::default()
            })
        })
        .filter(|e| !e.name.is_empty() && e.seconds != PREVIEW_SECONDS)
        .collect()
}

async fn scsearch(term: &str, results: u32) -> Result<Value, MediaError> {
    let query = format!("scsearch{results}:{}", term.trim());
    ytdlp::run_json(&["--flat-playlist", "-J", &query], SEARCH_LIMIT).await
}

/// Songs on SoundCloud for `term`, as a `tracks` section value.
pub async fn search(term: String) -> Result<Value, CatalogError> {
    let v = scsearch(&term, RESULTS)
        .await
        .map_err(|e| CatalogError::BadResponse(e.to_string()))?;
    Ok(json!({ "items": entries(&v) }))
}

/// The first result when its length is within [`MATCH_SECONDS`] of `seconds`.
pub(crate) fn pick_match(found: &[Entry], seconds: f64) -> Option<String> {
    let first = found.first()?;
    ((first.seconds as f64 - seconds).abs() <= MATCH_SECONDS).then(|| first.key.clone())
}

/// The SoundCloud id of "artist title" when its length matches (`seconds` must be known).
pub async fn find_match(artist: String, title: String, seconds: f64) -> Option<String> {
    if seconds <= 0.0 || title.trim().is_empty() {
        return None;
    }
    let v = scsearch(&format!("{artist} {title}"), 1).await.ok()?;
    pick_match(&entries(&v), seconds)
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("soundcloud_tests.rs");
}
