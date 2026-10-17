//! Normalising the typed adapter models (artist, album, playlist, watch pages) into [`Entry`] rows.
use super::Entry;
use serde_json::Value;

fn unwrap_variant(v: &Value) -> &Value {
    match v
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.values().next())
    {
        Some(inner) if inner.is_object() => inner,
        _ => v,
    }
}
fn string<'a>(v: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|k| v[*k].as_str()).unwrap_or("")
}
pub(super) fn names(v: &Value) -> String {
    v["artists"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}
pub(super) fn image(v: &Value) -> String {
    v["thumbnails"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .filter(|v| v["url"].as_str().is_some_and(image_url))
                .min_by_key(|v| (v["width"].as_i64().unwrap_or(360) - 360).abs())
        })
        .and_then(|v| v["url"].as_str())
        .unwrap_or("")
        .to_owned()
}
pub fn image_url(s: &str) -> bool {
    reqwest::Url::parse(s).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str().is_some_and(|h| {
                ["ytimg.com", "ggpht.com", "googleusercontent.com"]
                    .iter()
                    .any(|d| h == *d || h.ends_with(&format!(".{d}")))
            })
    })
}
pub(super) fn length(s: &str) -> u64 {
    if !s.contains(':') {
        return 0;
    }
    s.split(':')
        .try_fold(0u64, |n, p| {
            p.parse::<u64>()
                .ok()
                .and_then(|p| n.checked_mul(60)?.checked_add(p))
        })
        .unwrap_or(0)
}
pub(super) fn entry(v: &Value, kind: &str, parent: &Value) -> Option<Entry> {
    let v = unwrap_variant(v);
    let key = string(
        v,
        if kind == "track" {
            &["video_id"]
        } else if kind == "album" {
            &["album_id", "browse_id"]
        } else if kind == "artist" {
            &["browse_id", "channel_id"]
        } else {
            &["playlist_id"]
        },
    );
    if key.is_empty() {
        return None;
    }
    let mut artwork = image(v);
    if artwork.is_empty() {
        artwork = image(parent);
    }
    let mut performer = string(v, &["artist", "author", "channel_name"]).to_owned();
    if performer.is_empty() {
        performer = names(v);
    }
    if performer.is_empty() {
        performer = names(parent);
    }
    Some(Entry {
        key: key.into(),
        kind: kind.into(),
        name: string(v, &["title", "artist", "name"]).into(),
        performer,
        seconds: length(string(v, &["duration"])),
        artwork,
        album: v["album"]["name"]
            .as_str()
            .unwrap_or(if kind == "track" {
                parent["title"].as_str().unwrap_or("")
            } else {
                ""
            })
            .into(),
        artist_key: string(&v["artists"][0], &["id", "channel_id"]).into(),
        album_key: v["album"]["id"].as_str().unwrap_or("").into(),
    })
}
pub(super) fn collect_tracks(v: &Value, parent: &Value, out: &mut Vec<Entry>) {
    if let Some(t) = entry(v, "track", parent) {
        out.push(t);
        return;
    }
    match v {
        Value::Array(a) => {
            for x in a {
                collect_tracks(x, parent, out)
            }
        }
        Value::Object(o) => {
            for x in o.values() {
                collect_tracks(x, parent, out)
            }
        }
        _ => (),
    }
}
pub(super) fn collect_albums(v: &Value, out: &mut Vec<Entry>) {
    if v["thumbnails"].is_array()
        && (v["browse_id"]
            .as_str()
            .is_some_and(|s| s.starts_with("MPRE"))
            || v["album_id"].is_string())
    {
        if let Some(a) = entry(v, "album", &Value::Null) {
            out.push(a);
            return;
        }
    }
    match v {
        Value::Array(a) => {
            for x in a {
                collect_albums(x, out)
            }
        }
        Value::Object(o) => {
            for x in o.values() {
                collect_albums(x, out)
            }
        }
        _ => (),
    }
}
