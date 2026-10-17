//! Catalog `Entry` / JSON to domain model conversion.
use super::catalog::Entry;
use crate::model::*;
use serde_json::Value;

pub(super) fn cover(url: &str) -> Option<Cover> {
    (!url.is_empty()).then(|| Cover {
        url: url.into(),
        width: 360,
        height: 360,
    })
}

fn artist_refs(name: &str, id: &str) -> Vec<ArtistRef> {
    if name.is_empty() {
        vec![]
    } else {
        vec![ArtistRef {
            id: id.into(),
            name: name.into(),
        }]
    }
}

pub(super) fn track(e: &Entry) -> Track {
    Track {
        id: e.key.as_str().into(),
        title: e.name.as_str().into(),
        artists: artist_refs(&e.performer, &e.artist_key),
        album: (!e.album.is_empty()).then(|| AlbumRef {
            id: e.album_key.as_str().into(),
            title: e.album.as_str().into(),
        }),
        duration_ms: e.seconds * 1000,
        cover: cover(&e.artwork),
        explicit: false,
    }
}

pub(super) fn artist_item(e: Entry) -> ArtistItem {
    ArtistItem {
        id: e.key.into(),
        name: e.name.into(),
        cover: cover(&e.artwork),
    }
}

pub(super) fn album_item(e: Entry) -> AlbumItem {
    AlbumItem {
        artists: artist_refs(&e.performer, &e.artist_key),
        id: e.key.into(),
        title: e.name.into(),
        year: None,
        cover: cover(&e.artwork),
    }
}

pub(super) fn playlist_item(e: Entry) -> PlaylistItem {
    PlaylistItem {
        id: e.key.into(),
        title: e.name.into(),
        author: Some(e.performer).filter(|s| !s.is_empty()).map(Text::from),
        cover: cover(&e.artwork),
    }
}

/// Entries under `field` of a catalog response; malformed ones are skipped.
pub(super) fn entries(v: &Value, field: &str) -> Vec<Entry> {
    v[field]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            key: "abcdefghijk".into(),
            kind: "track".into(),
            name: "Song".into(),
            performer: "Artist".into(),
            seconds: 195,
            artwork: "https://i.ytimg.com/a.jpg".into(),
            album: "Album".into(),
            artist_key: "UC1".into(),
            album_key: "MPRE1".into(),
        }
    }

    #[test]
    fn converts_track_entry() {
        let t = track(&entry());
        assert_eq!(t.id, "abcdefghijk");
        assert_eq!(t.duration_ms, 195_000);
        assert_eq!(t.artists_label(), "Artist");
        assert_eq!(t.album.unwrap().id, "MPRE1");
        assert!(t.cover.is_some());
    }

    #[test]
    fn missing_fields_become_none_or_empty() {
        let mut e = entry();
        e.performer.clear();
        e.album.clear();
        e.artwork.clear();
        let t = track(&e);
        assert!(t.artists.is_empty() && t.album.is_none() && t.cover.is_none());
        assert_eq!(playlist_item(e).author, None);
    }

    #[test]
    fn skips_malformed_entries() {
        let v = serde_json::json!({"items": [serde_json::to_value(entry()).unwrap(), {"nope": 1}]});
        assert_eq!(entries(&v, "items").len(), 1);
        assert!(entries(&v, "missing").is_empty());
    }
}
