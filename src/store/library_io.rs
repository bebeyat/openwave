//! Reading and writing `library.json`: item-by-item salvage of damaged files, the v0 migration, the
//! separate `last-queue.json` and the caps applied after loading.
use super::library::{
    HistoryEntry, Library, HISTORY_CAP, LAST_QUEUE_FILE, QUEUE_PERSIST_CAP, SEARCH_CAP,
};
use super::persist::{self, Read};
use super::versioned::{LoadReport, Sidecar, Versioned};
use crate::{
    model::{Id, Playlist, Track},
    player::queue::QueueSnapshot,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::path::Path;

/// A damaged queue snapshot must not cost the user their playlists: drop it instead.
pub(super) fn lenient_queue<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<QueueSnapshot>, D::Error> {
    Ok(QueueSnapshot::deserialize(Value::deserialize(d)?).ok())
}

/// Keep at most [`QUEUE_PERSIST_CAP`] entries, as a window that contains the current one.
pub fn cap_queue(snapshot: QueueSnapshot) -> QueueSnapshot {
    let len = snapshot.tracks.len();
    if len <= QUEUE_PERSIST_CAP {
        return snapshot;
    }
    let current = snapshot.current.unwrap_or(0).min(len - 1);
    let start = current
        .saturating_sub(QUEUE_PERSIST_CAP / 5)
        .min(len - QUEUE_PERSIST_CAP);
    QueueSnapshot {
        tracks: snapshot.tracks[start..start + QUEUE_PERSIST_CAP].to_vec(),
        current: snapshot
            .current
            .map(|c| c.saturating_sub(start).min(QUEUE_PERSIST_CAP - 1)),
        ..snapshot
    }
}

/// Decode `value` item by item; undecodable items go to `dropped`.
fn salvage_items<T: DeserializeOwned>(
    value: Option<&Value>,
    dropped: &mut Vec<Value>,
    keep: impl Fn(&T) -> bool,
) -> Vec<T> {
    match value {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|raw| match serde_json::from_value::<T>(raw.clone()) {
                Ok(item) if keep(&item) => Some(item),
                _ => {
                    dropped.push(raw.clone());
                    None
                }
            })
            .collect(),
        Some(other) => {
            dropped.push(other.clone());
            vec![]
        }
    }
}

fn has_id(t: &Track) -> bool {
    !t.id.is_empty()
}

fn salvage_playlists(value: Option<&Value>, dropped: &mut Vec<Value>) -> Vec<Playlist> {
    let Some(Value::Array(items)) = value else {
        return salvage_items(value, dropped, |_: &Playlist| true);
    };
    items
        .iter()
        .filter_map(|raw| {
            if let Ok(p) = serde_json::from_value::<Playlist>(raw.clone()) {
                return Some(p);
            }
            // Keep the playlist itself when only some of its tracks are bad.
            let o = raw.as_object()?;
            let id = o
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            let Some(id) = id else {
                dropped.push(raw.clone());
                return None;
            };
            let text = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or_default();
            let num = |k: &str| o.get(k).and_then(Value::as_u64).unwrap_or_default();
            Some(Playlist {
                id: id.into(),
                name: text("name").into(),
                tracks: salvage_items(o.get("tracks"), dropped, has_id),
                created_ms: num("created_ms"),
                updated_ms: num("updated_ms"),
                cover: Some(text("cover").to_owned()).filter(|c| !c.is_empty()),
            })
        })
        .collect()
}

impl Versioned for Library {
    const FILE: &'static str = "library.json";
    const VERSION: u32 = 1;

    /// v0 files called the liked list `likes` and the history `recent`.
    fn migrate(mut doc: Value, from: u32) -> Value {
        let Some(o) = doc.as_object_mut() else {
            return doc;
        };
        if from == 0 {
            for (old, new) in [("likes", "liked"), ("recent", "history")] {
                if let Some(v) = o.remove(old) {
                    o.entry(new).or_insert(v);
                }
            }
        }
        doc
    }

    /// One bad playlist, track, like or history entry must not cost the user everything else.
    fn salvage(doc: &Value, dropped: &mut Vec<Value>) -> Option<Self> {
        let o = doc.as_object()?;
        Some(Self {
            playlists: salvage_playlists(o.get("playlists"), dropped),
            liked: salvage_items(o.get("liked"), dropped, has_id),
            history: salvage_items(o.get("history"), dropped, |h: &HistoryEntry| {
                has_id(&h.track)
            }),
            recent_searches: salvage_items(o.get("recent_searches"), dropped, |_: &String| true),
            last_queue: o
                .get("last_queue")
                .and_then(|v| QueueSnapshot::deserialize(v).ok()),
            local_files: salvage_items(o.get("local_files"), dropped, has_id),
            lyric_offsets: salvage_items(o.get("lyric_offsets"), dropped, |_: &(Id, i32)| true),
        })
    }

    fn sidecars(&self) -> Vec<Sidecar> {
        vec![Sidecar {
            file: LAST_QUEUE_FILE,
            value: self
                .last_queue
                .as_ref()
                .and_then(|q| serde_json::to_value(q).ok()),
        }]
    }

    /// `last-queue.json` wins over the old `last_queue` field of `library.json` (migration).
    fn with_sidecars(self, dir: &Path, report: &mut LoadReport) -> Self {
        match persist::read_json(dir, LAST_QUEUE_FILE) {
            Read::Value(v) => Self {
                last_queue: QueueSnapshot::deserialize(v).ok(),
                ..self
            },
            Read::Unreadable { .. } => {
                // Writing would replace a queue we could not read.
                report.read_only = true;
                self
            }
            Read::Missing | Read::Corrupt { .. } => self,
        }
    }

    fn sanitize(mut self) -> Self {
        self.last_queue = self.last_queue.map(cap_queue);
        self.history.truncate(HISTORY_CAP);
        self.recent_searches.truncate(SEARCH_CAP);
        self
    }
}
