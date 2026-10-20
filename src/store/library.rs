//! The user's library (`library.json`): playlists, likes, history, recent searches, the last
//! queue and local files. All operations are pure: they take `&self` and return a new `Library`.
pub use super::library_io::cap_queue;
use crate::{
    model::{Id, Playlist, Text, Track},
    player::queue::QueueSnapshot,
};
use serde::{Deserialize, Serialize};

/// The last queue lives in its own small file so the 10-second position saves never rewrite the
/// whole library.
pub const LAST_QUEUE_FILE: &str = "last-queue.json";
/// At most this many queue entries are persisted (a window around the current one).
pub const QUEUE_PERSIST_CAP: usize = 500;
pub const HISTORY_CAP: usize = 200;
pub const SEARCH_CAP: usize = 12;
pub const PLAYLIST_NAME_MAX: usize = 120;
pub const LYRIC_OFFSET_CAP: usize = 300;
pub const LYRIC_OFFSET_MAX_MS: i32 = 30_000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub track: Track,
    pub played_at_ms: u64,
    /// How many times the track was played; 0 in files written before plays were counted.
    pub plays: u32,
}

impl HistoryEntry {
    /// Plays, counting entries from older files (no count) as one.
    pub fn play_count(&self) -> u32 {
        self.plays.max(1)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub playlists: Vec<Playlist>,
    /// Newest like first.
    pub liked: Vec<Track>,
    /// Most recent play first; one entry per track, capped at [`HISTORY_CAP`].
    pub history: Vec<HistoryEntry>,
    /// Most recent search first, case-insensitively unique, capped at [`SEARCH_CAP`].
    pub recent_searches: Vec<String>,
    /// Persisted in [`LAST_QUEUE_FILE`]; still read from `library.json` of older versions.
    #[serde(
        deserialize_with = "super::library_io::lenient_queue",
        skip_serializing
    )]
    pub last_queue: Option<QueueSnapshot>,
    pub local_files: Vec<Track>,
    /// Per-track lyrics shift in ms (positive: lines later), most recently set first.
    pub lyric_offsets: Vec<(Id, i32)>,
}

fn clean_name(name: &str) -> Text {
    name.trim()
        .chars()
        .take(PLAYLIST_NAME_MAX)
        .collect::<String>()
        .into()
}

fn with_playlist(lib: &Library, id: &str, f: impl Fn(&Playlist) -> Playlist) -> Library {
    Library {
        playlists: lib
            .playlists
            .iter()
            .map(|p| if p.id == id { f(p) } else { p.clone() })
            .collect(),
        ..lib.clone()
    }
}

impl Library {
    pub fn playlist(&self, id: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    /// Append an empty playlist. The caller supplies a unique `id` (see [`new_id`]) and the time.
    /// The UI is responsible for passing a non-empty, translated name.
    pub fn create_playlist(&self, id: &str, name: &str, now_ms: u64) -> Library {
        let playlist = Playlist {
            id: id.into(),
            name: clean_name(name),
            tracks: vec![],
            created_ms: now_ms,
            updated_ms: now_ms,
            cover: None,
        };
        Library {
            playlists: [self.playlists.clone(), vec![playlist]].concat(),
            ..self.clone()
        }
    }

    /// Set (or with `None` remove) the picture of a playlist.
    pub fn set_playlist_cover(&self, id: &str, cover: Option<String>, now_ms: u64) -> Library {
        with_playlist(self, id, |p| Playlist {
            cover: cover.clone(),
            updated_ms: now_ms,
            ..p.clone()
        })
    }

    pub fn rename_playlist(&self, id: &str, name: &str, now_ms: u64) -> Library {
        let name = clean_name(name);
        if name.is_empty() {
            return self.clone();
        }
        with_playlist(self, id, |p| Playlist {
            name: name.clone(),
            updated_ms: now_ms,
            ..p.clone()
        })
    }

    pub fn delete_playlist(&self, id: &str) -> Library {
        Library {
            playlists: self
                .playlists
                .iter()
                .filter(|p| p.id != id)
                .cloned()
                .collect(),
            ..self.clone()
        }
    }

    /// Append tracks that are not already in the playlist (a track appears once per playlist).
    pub fn add_to_playlist(&self, id: &str, tracks: &[Track], now_ms: u64) -> Library {
        with_playlist(self, id, |p| {
            let mut next = p.tracks.clone();
            for t in tracks {
                if !next.iter().any(|x| x.id == t.id) {
                    next.push(t.clone());
                }
            }
            Playlist {
                tracks: next,
                updated_ms: now_ms,
                ..p.clone()
            }
        })
    }

    pub fn remove_from_playlist(&self, id: &str, track_id: &str, now_ms: u64) -> Library {
        with_playlist(self, id, |p| Playlist {
            tracks: p
                .tracks
                .iter()
                .filter(|t| t.id != track_id)
                .cloned()
                .collect(),
            updated_ms: now_ms,
            ..p.clone()
        })
    }

    /// Move the track at index `from` so that it ends up at index `to` (drag-reorder).
    /// Out-of-range indexes leave the playlist unchanged.
    pub fn move_in_playlist(&self, id: &str, from: usize, to: usize, now_ms: u64) -> Library {
        with_playlist(self, id, |p| {
            if from >= p.tracks.len() || to >= p.tracks.len() || from == to {
                return p.clone();
            }
            let mut tracks = p.tracks.clone();
            let item = tracks.remove(from);
            tracks.insert(to, item);
            Playlist {
                tracks,
                updated_ms: now_ms,
                ..p.clone()
            }
        })
    }

    pub fn is_liked(&self, track_id: &str) -> bool {
        self.liked.iter().any(|t| t.id == track_id)
    }

    /// Like the track (newest first) or remove the like if it was already liked.
    pub fn toggle_like(&self, track: &Track) -> Library {
        let liked = if self.is_liked(&track.id) {
            self.liked
                .iter()
                .filter(|t| t.id != track.id)
                .cloned()
                .collect()
        } else {
            [vec![track.clone()], self.liked.clone()].concat()
        };
        Library {
            liked,
            ..self.clone()
        }
    }

    /// Put the track at the top of the history (replacing its older entry, keeping its count).
    pub fn record_play(&self, track: &Track, now_ms: u64) -> Library {
        let earlier = self
            .history
            .iter()
            .find(|h| h.track.id == track.id)
            .map_or(0, HistoryEntry::play_count);
        let entry = HistoryEntry {
            track: track.clone(),
            played_at_ms: now_ms,
            plays: earlier + 1,
        };
        let history = std::iter::once(entry)
            .chain(
                self.history
                    .iter()
                    .filter(|h| h.track.id != track.id)
                    .cloned(),
            )
            .take(HISTORY_CAP)
            .collect();
        Library {
            history,
            ..self.clone()
        }
    }

    pub fn clear_history(&self) -> Library {
        Library {
            history: vec![],
            ..self.clone()
        }
    }

    pub fn remember_search(&self, term: &str) -> Library {
        let term = term.trim();
        if term.is_empty() {
            return self.clone();
        }
        let same = |s: &str| s.to_lowercase() == term.to_lowercase();
        let searches = std::iter::once(term.to_owned())
            .chain(self.recent_searches.iter().filter(|s| !same(s)).cloned())
            .take(SEARCH_CAP)
            .collect();
        Library {
            recent_searches: searches,
            ..self.clone()
        }
    }

    pub fn forget_search(&self, term: &str) -> Library {
        Library {
            recent_searches: self
                .recent_searches
                .iter()
                .filter(|s| *s != term)
                .cloned()
                .collect(),
            ..self.clone()
        }
    }

    pub fn clear_searches(&self) -> Library {
        Library {
            recent_searches: vec![],
            ..self.clone()
        }
    }

    pub fn lyric_offset(&self, track_id: &str) -> i32 {
        self.lyric_offsets
            .iter()
            .find(|(id, _)| *id == track_id)
            .map_or(0, |(_, ms)| *ms)
    }

    /// Remember the lyrics shift of a track (0 forgets it); bounded to [`LYRIC_OFFSET_CAP`].
    pub fn set_lyric_offset(&self, track_id: &str, ms: i32) -> Library {
        let ms = ms.clamp(-LYRIC_OFFSET_MAX_MS, LYRIC_OFFSET_MAX_MS);
        let rest = self.lyric_offsets.iter().filter(|(id, _)| *id != track_id);
        let lyric_offsets = (ms != 0)
            .then(|| (Id::from(track_id), ms))
            .into_iter()
            .chain(rest.cloned())
            .take(LYRIC_OFFSET_CAP)
            .collect();
        Library {
            lyric_offsets,
            ..self.clone()
        }
    }

    pub fn set_last_queue(&self, snapshot: Option<QueueSnapshot>) -> Library {
        Library {
            last_queue: snapshot.map(cap_queue),
            ..self.clone()
        }
    }

    pub fn add_local_files(&self, tracks: &[Track]) -> Library {
        let mut files = self.local_files.clone();
        for t in tracks {
            if !files.iter().any(|f| f.id == t.id) {
                files.push(t.clone());
            }
        }
        Library {
            local_files: files,
            ..self.clone()
        }
    }

    /// Replace the stored copies of these local tracks (new tags) wherever they appear: the local
    /// list, likes, playlists and history.
    pub fn refresh_local_tracks(&self, fresh: &[Track]) -> Library {
        let pick = |t: &Track| {
            fresh
                .iter()
                .find(|f| f.id == t.id)
                .cloned()
                .unwrap_or_else(|| t.clone())
        };
        Library {
            local_files: self.local_files.iter().map(pick).collect(),
            liked: self.liked.iter().map(pick).collect(),
            playlists: self
                .playlists
                .iter()
                .map(|p| crate::model::Playlist {
                    tracks: p.tracks.iter().map(pick).collect(),
                    ..p.clone()
                })
                .collect(),
            history: self
                .history
                .iter()
                .map(|h| HistoryEntry {
                    track: pick(&h.track),
                    ..h.clone()
                })
                .collect(),
            ..self.clone()
        }
    }

    pub fn remove_local_file(&self, track_id: &str) -> Library {
        Library {
            local_files: self
                .local_files
                .iter()
                .filter(|t| t.id != track_id)
                .cloned()
                .collect(),
            ..self.clone()
        }
    }
}

/// Fresh random id for a user playlist.
pub fn new_id() -> Id {
    uuid::Uuid::new_v4().to_string().into()
}

/// Unix time in milliseconds.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

// Local-only tests (see build.rs).
#[cfg(all(test, private_tests))]
mod offset_tests {
    include!("library_offset_tests.rs");
}
