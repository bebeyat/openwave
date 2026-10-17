//! Domain types shared by the backend, the stores and the UI.
//!
//! Conventions: every type is `Clone + Serialize + Deserialize`, ids are opaque strings wrapped in
//! [`Text`] (an `Arc<str>`, so cloning a track copies pointers, not text), durations are
//! milliseconds, timestamps are Unix milliseconds. Ids are the catalog's own ids (YouTube video id
//! for tracks, `UC…` for artists, `MPRE…` for albums, `VL…`/`PL…` for lists) or `local-<uuid>` for
//! files picked from disk and a random uuid for user playlists.
// The UI consumes these types incrementally; unused items are expected until it does.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref, sync::Arc};

/// Immutable shared string: cheap to clone, serialized as a plain JSON string.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text(Arc<str>);

impl Text {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Deref for Text {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text(Arc::from(s))
    }
}
impl From<String> for Text {
    fn from(s: String) -> Self {
        Text(Arc::from(s))
    }
}
impl From<&String> for Text {
    fn from(s: &String) -> Self {
        Text(Arc::from(s.as_str()))
    }
}
impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}
impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        &*self.0 == other.as_str()
    }
}
impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}
impl Serialize for Text {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Text::from)
    }
}

/// Stable identifier of a catalog entity or user object.
pub type Id = Text;

/// Cover image reference (an https URL on an allowed image host, see `docs/PRIVACY.md`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    pub url: Text,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: Id,
    pub name: Text,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumRef {
    pub id: Id,
    pub title: Text,
}

/// A playable item. Local files have ids starting with `local-` and usually no artists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub id: Id,
    pub title: Text,
    pub artists: Vec<ArtistRef>,
    pub album: Option<AlbumRef>,
    pub duration_ms: u64,
    pub cover: Option<Cover>,
    pub explicit: bool,
}

impl Track {
    /// "A, B" label of all artists (empty when unknown).
    pub fn artists_label(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
    pub fn is_local(&self) -> bool {
        self.id.starts_with("local-")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistItem {
    pub id: Id,
    pub name: Text,
    pub cover: Option<Cover>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumItem {
    pub id: Id,
    pub title: Text,
    pub artists: Vec<ArtistRef>,
    pub year: Option<u16>,
    pub cover: Option<Cover>,
}

/// A named catalog entry point: a genre (`key` = its browse params) or one of a genre's
/// playlists (`key` = its `VL…` id).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogLink {
    pub name: String,
    pub key: String,
}

/// A catalog (online) playlist as shown in search results.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: Id,
    pub title: Text,
    pub author: Option<Text>,
    pub cover: Option<Cover>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<ArtistItem>,
    pub albums: Vec<AlbumItem>,
    pub playlists: Vec<PlaylistItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistDetails {
    pub id: Id,
    pub name: Text,
    pub bio: String,
    pub cover: Option<Cover>,
    pub tracks: Vec<Track>,
    pub albums: Vec<AlbumItem>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    #[default]
    Album,
    Playlist,
}

/// An album or an online playlist with its tracks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionDetails {
    pub kind: CollectionKind,
    pub id: Id,
    pub title: Text,
    pub author: String,
    pub description: String,
    pub cover: Option<Cover>,
    pub tracks: Vec<Track>,
}

/// One word of an enhanced-LRC line. `end_ms` is the next word's start (or the line end).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricLine {
    pub start_ms: u64,
    pub text: String,
    pub words: Vec<LyricWord>,
}

/// Lyrics for a track: time-synced lines when available, plain text otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lyrics {
    pub plain: String,
    pub synced: Vec<LyricLine>,
    pub provider: String,
}

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.plain.trim().is_empty() && self.synced.is_empty()
    }
    pub fn is_synced(&self) -> bool {
        !self.synced.is_empty()
    }
    /// Index of the line that is current at `position_ms` (last line starting at or before it).
    #[cfg(test)]
    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        self.synced
            .partition_point(|l| l.start_ms <= position_ms)
            .checked_sub(1)
    }
}

/// A user playlist stored in the library.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    pub id: Id,
    pub name: Text,
    pub tracks: Vec<Track>,
    pub created_ms: u64,
    pub updated_ms: u64,
    /// A picture the user chose (`owcover:<hash>`); `None` shows the mosaic of the tracks.
    pub cover: Option<String>,
}

impl Playlist {
    #[cfg(test)]
    pub fn duration_ms(&self) -> u64 {
        self.tracks.iter().map(|t| t.duration_ms).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, artists: &[&str]) -> Track {
        Track {
            id: id.into(),
            title: "T".into(),
            artists: artists
                .iter()
                .map(|n| ArtistRef {
                    id: "".into(),
                    name: (*n).into(),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn text_serializes_as_plain_string() {
        let json = serde_json::to_string(&Text::from("abc")).unwrap();
        assert_eq!(json, "\"abc\"");
        assert_eq!(serde_json::from_str::<Text>(&json).unwrap(), "abc");
    }

    #[test]
    fn track_round_trips_and_tolerates_missing_fields() {
        let t = track("abcdefghijk", &["A", "B"]);
        let back: Track = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        let sparse: Track = serde_json::from_str(r#"{"id":"x","extra":1}"#).unwrap();
        assert_eq!(sparse.id, "x");
        assert!(sparse.artists.is_empty());
    }

    #[test]
    fn artist_label_and_local_flag() {
        assert_eq!(track("a", &["A", "B"]).artists_label(), "A, B");
        assert_eq!(track("a", &[]).artists_label(), "");
        assert!(track("local-1", &[]).is_local());
        assert!(!track("abc", &[]).is_local());
    }

    #[test]
    fn current_lyric_line_lookup() {
        let l = Lyrics {
            synced: [1000u64, 2000, 3000]
                .iter()
                .map(|&s| LyricLine {
                    start_ms: s,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        assert_eq!(l.line_at(0), None);
        assert_eq!(l.line_at(1000), Some(0));
        assert_eq!(l.line_at(2999), Some(1));
        assert_eq!(l.line_at(99_999), Some(2));
    }

    #[test]
    fn playlist_duration_sums_tracks() {
        let mut a = track("a", &[]);
        a.duration_ms = 1000;
        let p = Playlist {
            tracks: vec![a.clone(), a],
            ..Default::default()
        };
        assert_eq!(p.duration_ms(), 2000);
    }
}
