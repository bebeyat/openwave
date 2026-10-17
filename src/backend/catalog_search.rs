//! Reading search results: `musicResponsiveListItemRenderer` rows of the raw search response.
use super::{
    parse::{image, length},
    Entry,
};
use serde_json::Value;

const FLEX: &str = "musicResponsiveListItemFlexColumnRenderer";
const PLAY_ENDPOINT: &str =
    "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint";

fn text<'a>(v: &'a Value, pointer: &str) -> Option<&'a str> {
    v.pointer(pointer).and_then(Value::as_str)
}

/// Id of the song (video id) or of the album, artist or playlist behind a row.
fn row_key<'a>(row: &'a Value, kind: &str) -> &'a str {
    if kind == "track" {
        text(row, "/playlistItemData/videoId")
            .or_else(|| {
                text(
                    row,
                    &format!("/flexColumns/0/{FLEX}/text/runs/0/navigationEndpoint/watchEndpoint/videoId"),
                )
            })
            .or_else(|| text(row, &format!("{PLAY_ENDPOINT}/watchEndpoint/videoId")))
    } else {
        text(row, "/navigationEndpoint/browseEndpoint/browseId").or_else(|| {
            text(row, &format!("{PLAY_ENDPOINT}/watchPlaylistEndpoint/playlistId"))
        })
    }
    .unwrap_or("")
}

/// What the second line of a row ("Artist • Album • 3:15") links to.
#[derive(Default)]
struct Subtitle {
    full: String,
    artist_key: String,
    album_key: String,
    album: String,
    artists: Vec<String>,
}

impl Subtitle {
    fn parse(row: &Value) -> Self {
        let mut sub = Subtitle::default();
        let runs = row
            .pointer(&format!("/flexColumns/1/{FLEX}/text/runs"))
            .and_then(Value::as_array);
        for run in runs.into_iter().flatten() {
            let txt = run["text"].as_str().unwrap_or("");
            sub.full.push_str(txt);
            if let Some(id) = text(run, "/navigationEndpoint/browseEndpoint/browseId") {
                sub.add_link(id, txt.trim());
            }
        }
        sub
    }

    fn add_link(&mut self, id: &str, label: &str) {
        if id.starts_with("UC") {
            if self.artist_key.is_empty() {
                self.artist_key = id.to_owned();
            }
            if !label.is_empty() {
                self.artists.push(label.to_owned());
            }
        } else if id.starts_with("MPRE") {
            if self.album_key.is_empty() {
                self.album_key = id.to_owned();
            }
            if self.album.is_empty() && !label.is_empty() {
                self.album = label.to_owned();
            }
        }
    }

    fn parts(&self) -> Vec<&str> {
        self.full
            .split(" • ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn performer_of(sub: &Subtitle, parts: &[&str], kind: &str) -> String {
    if !sub.artists.is_empty() {
        return sub.artists.join(", ");
    }
    let pick = match kind {
        "track" => parts
            .iter()
            .copied()
            .find(|p| !matches!(*p, "Song" | "Video" | "Şarkı") && length(p) == 0),
        "album" => parts.get(1).or(parts.first()).copied(),
        _ => parts.first().copied(),
    };
    pick.unwrap_or("").to_owned()
}

/// A track row's length is the last part that looks like a duration.
fn track_seconds(parts: &[&str]) -> u64 {
    parts
        .iter()
        .rev()
        .find_map(|p| {
            let s = length(p);
            (s > 0).then_some(s)
        })
        .unwrap_or(0)
}

pub(super) fn mrlir_entry(row: &Value, kind: &str) -> Option<Entry> {
    let name = text(row, &format!("/flexColumns/0/{FLEX}/text/runs/0/text"))
        .unwrap_or("")
        .trim();
    let key = row_key(row, kind);
    if name.is_empty() || key.is_empty() {
        return None;
    }
    let sub = Subtitle::parse(row);
    let parts = sub.parts();
    let performer = performer_of(&sub, &parts, kind);
    let mut album = sub.album.clone();
    if kind == "track" && album.is_empty() && parts.len() >= 3 {
        let candidate = parts[parts.len() - 2];
        if candidate != performer && length(candidate) == 0 {
            album = candidate.to_owned();
        }
    }
    Some(Entry {
        key: key.into(),
        kind: kind.into(),
        name: name.into(),
        seconds: if kind == "track" {
            track_seconds(&parts)
        } else {
            0
        },
        artwork: row
            .pointer("/thumbnail/musicThumbnailRenderer/thumbnail")
            .map(image)
            .unwrap_or_default(),
        performer,
        album,
        artist_key: sub.artist_key,
        album_key: sub.album_key,
    })
}

pub(super) fn collect_mrlir(v: &Value, kind: &str, out: &mut Vec<Entry>) {
    if let Some(mrlir) = v.get("musicResponsiveListItemRenderer") {
        if let Some(e) = mrlir_entry(mrlir, kind) {
            out.push(e);
        }
        return;
    }
    match v {
        Value::Array(a) => {
            for x in a {
                collect_mrlir(x, kind, out);
            }
        }
        Value::Object(o) => {
            for x in o.values() {
                collect_mrlir(x, kind, out);
            }
        }
        _ => (),
    }
}
