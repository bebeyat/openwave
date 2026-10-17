//! Play queue as a pure state machine: no UI, no audio, no I/O.
//!
//! Every operation takes `&self` and returns a new [`Queue`] (or a [`Step`] that carries one plus
//! what the player should do). Entries get a unique `uid`, so the same track can be queued twice
//! and lists can be keyed/dragged by uid.
//!
//! * **Order**: `entries()` is the play order. With shuffle on it is the shuffled order; the
//!   original order is kept internally and restored when shuffle is turned off.
//! * **Shuffle** is seeded (`seed: u64`, the caller passes e.g. [`fresh_seed`]) so it is
//!   reproducible in tests. The current entry stays first, and no track is played twice in a
//!   row unless it is the only one in the queue (also across the repeat-all wrap-around).
//! * **Previous** walks back through what was actually played (history of uids), then falls back
//!   to positional order. The "restart if more than 3 s in" rule belongs to the player.
//! * **Repeat one** only applies to a track that ended by itself (`next(true)`); pressing Next
//!   moves on as if repeat were off.
use crate::model::Track;
use serde::{Deserialize, Serialize};

#[path = "queue_ops.rs"]
mod ops;
#[path = "queue_shuffle.rs"]
mod shuffle;

pub use shuffle::fresh_seed;
use shuffle::shuffled;

const HISTORY_CAP: usize = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueEntry {
    pub uid: u64,
    pub track: Track,
}

/// What the player should do after a transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Load and play the (new) current entry.
    Play,
    /// Seek the current entry to 0 and play it again.
    Restart,
    /// Nothing more to play: pause (or fetch similar tracks when autoplay is on).
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub queue: Queue,
    pub action: Action,
}

/// Serializable form stored in the library (`last_queue`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueSnapshot {
    /// Tracks in play order.
    pub tracks: Vec<Track>,
    pub current: Option<usize>,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub position_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    entries: Vec<QueueEntry>,
    current: Option<usize>,
    repeat: Repeat,
    shuffle: bool,
    /// Uids in original order; only maintained while shuffled.
    natural: Vec<u64>,
    /// Uids of entries already played, oldest first.
    history: Vec<u64>,
    seed: u64,
    next_uid: u64,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- reading -------------------------------------------------------------------------
    pub fn entries(&self) -> &[QueueEntry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }
    pub fn current(&self) -> Option<&QueueEntry> {
        self.current.and_then(|i| self.entries.get(i))
    }
    pub fn current_uid(&self) -> Option<u64> {
        self.current().map(|e| e.uid)
    }
    /// Entries after the current one ("Sıradakiler"); everything when nothing is current.
    pub fn upcoming(&self) -> &[QueueEntry] {
        let from = self.current.map_or(0, |i| i + 1);
        self.entries.get(from..).unwrap_or(&[])
    }
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    pub fn tracks(&self) -> Vec<Track> {
        self.entries.iter().map(|e| e.track.clone()).collect()
    }

    // ---- building ------------------------------------------------------------------------
    fn fresh_entries(&self, tracks: &[Track]) -> (Vec<QueueEntry>, u64) {
        let entries = tracks
            .iter()
            .zip(self.next_uid..)
            .map(|(track, uid)| QueueEntry {
                uid,
                track: track.clone(),
            })
            .collect::<Vec<_>>();
        (entries, self.next_uid + tracks.len() as u64)
    }

    /// Replace the queue with `tracks` and start at index `start` (clamped). History is reset.
    /// Repeat and shuffle settings are kept; with shuffle on, `start` plays first and the rest is
    /// shuffled with `seed`.
    pub fn play_now(&self, tracks: &[Track], start: usize, seed: u64) -> Queue {
        let (entries, next_uid) = self.fresh_entries(tracks);
        if entries.is_empty() {
            return Queue {
                repeat: self.repeat,
                shuffle: self.shuffle,
                next_uid,
                seed,
                ..Queue::default()
            };
        }
        let start = start.min(entries.len() - 1);
        let base = Queue {
            entries,
            current: Some(start),
            history: vec![],
            natural: vec![],
            next_uid,
            seed,
            ..self.clone()
        };
        if self.shuffle {
            base.reshuffled_around_current()
        } else {
            base
        }
    }

    /// Current entry first, everything else shuffled; `natural` records the original order.
    fn reshuffled_around_current(&self) -> Queue {
        let natural: Vec<u64> = self.entries.iter().map(|e| e.uid).collect();
        let mut state = self.seed;
        let (head, rest): (Vec<_>, Vec<_>) = self
            .entries
            .iter()
            .cloned()
            .partition(|e| Some(e.uid) == self.current_uid());
        let avoid = head.first().map(|e| e.track.id.to_string());
        let entries = [head.clone(), shuffled(rest, &mut state, avoid.as_deref())].concat();
        Queue {
            current: head.first().map(|_| 0),
            entries,
            natural,
            seed: state,
            ..self.clone()
        }
    }

    /// Insert tracks right after the current one (before earlier "play next" additions).
    pub fn add_next(&self, tracks: &[Track]) -> Queue {
        let (new, next_uid) = self.fresh_entries(tracks);
        let at = self.current.map_or(0, |i| i + 1).min(self.entries.len());
        let anchor = self.current_uid();
        let entries = [&self.entries[..at], &new[..], &self.entries[at..]].concat();
        let natural = if self.shuffle {
            let pos = anchor
                .and_then(|u| self.natural.iter().position(|n| *n == u))
                .map_or(0, |p| p + 1);
            let ids = new.iter().map(|e| e.uid);
            [
                &self.natural[..pos],
                &ids.collect::<Vec<_>>()[..],
                &self.natural[pos..],
            ]
            .concat()
        } else {
            vec![]
        };
        Queue {
            entries,
            natural,
            next_uid,
            ..self.clone()
        }
    }

    /// Append tracks at the end of the queue.
    pub fn add_last(&self, tracks: &[Track]) -> Queue {
        let (new, next_uid) = self.fresh_entries(tracks);
        let natural = if self.shuffle {
            [self.natural.clone(), new.iter().map(|e| e.uid).collect()].concat()
        } else {
            vec![]
        };
        Queue {
            entries: [self.entries.clone(), new].concat(),
            natural,
            next_uid,
            ..self.clone()
        }
    }

    // ---- moving --------------------------------------------------------------------------
    fn with_current(&self, index: usize, history: Vec<u64>) -> Queue {
        Queue {
            current: Some(index),
            history,
            ..self.clone()
        }
    }

    fn pushed_history(&self) -> Vec<u64> {
        let mut h = self.history.clone();
        if let Some(uid) = self.current_uid() {
            h.push(uid);
        }
        let excess = h.len().saturating_sub(HISTORY_CAP);
        h.drain(..excess);
        h
    }

    fn step(queue: Queue, action: Action) -> Step {
        Step { queue, action }
    }

    /// Jump to a specific entry (click in the queue list).
    pub fn play_uid(&self, uid: u64) -> Step {
        match self.entries.iter().position(|e| e.uid == uid) {
            Some(i) if Some(i) == self.current => Self::step(self.clone(), Action::Restart),
            Some(i) => Self::step(self.with_current(i, self.pushed_history()), Action::Play),
            None => Self::step(self.clone(), Action::Stop),
        }
    }

    /// Next track. `auto` is true when the current track ended by itself (repeat-one applies).
    pub fn next(&self, auto: bool) -> Step {
        let Some(cur) = self.current else {
            return match self.entries.is_empty() {
                true => Self::step(self.clone(), Action::Stop),
                false => Self::step(self.with_current(0, vec![]), Action::Play),
            };
        };
        if auto && self.repeat == Repeat::One {
            return Self::step(self.clone(), Action::Restart);
        }
        if cur + 1 < self.len() {
            return Self::step(
                self.with_current(cur + 1, self.pushed_history()),
                Action::Play,
            );
        }
        if self.repeat != Repeat::All {
            return Self::step(self.clone(), Action::Stop);
        }
        if self.len() == 1 {
            return Self::step(self.clone(), Action::Restart);
        }
        Self::step(self.wrapped(), Action::Play)
    }

    /// Repeat-all wrap-around: start over from the top (reshuffled when shuffle is on).
    fn wrapped(&self) -> Queue {
        let history = self.pushed_history();
        if !self.shuffle {
            return self.with_current(0, history);
        }
        let mut state = self.seed;
        let avoid = self.current().map(|e| e.track.id.to_string());
        let entries = shuffled(self.entries.clone(), &mut state, avoid.as_deref());
        Queue {
            entries,
            current: Some(0),
            history,
            seed: state,
            ..self.clone()
        }
    }

    /// Previous track: the last one actually played, else the one before in the list; at the very
    /// start it wraps (repeat-all, not shuffled) or asks to restart the current track.
    pub fn previous(&self) -> Step {
        let Some(cur) = self.current else {
            return Self::step(self.clone(), Action::Stop);
        };
        let mut history = self.history.clone();
        while let Some(uid) = history.pop() {
            if let Some(i) = self
                .entries
                .iter()
                .position(|e| e.uid == uid)
                .filter(|i| *i != cur)
            {
                return Self::step(self.with_current(i, history), Action::Play);
            }
        }
        if !self.shuffle && cur > 0 {
            return Self::step(self.with_current(cur - 1, vec![]), Action::Play);
        }
        if !self.shuffle && self.repeat == Repeat::All && self.len() > 1 {
            return Self::step(self.with_current(self.len() - 1, vec![]), Action::Play);
        }
        Self::step(self.clone(), Action::Restart)
    }

    // ---- persistence ---------------------------------------------------------------------
    pub fn snapshot(&self, position_ms: u64) -> QueueSnapshot {
        QueueSnapshot {
            tracks: self.tracks(),
            current: self.current,
            repeat: self.repeat,
            shuffle: self.shuffle,
            position_ms,
        }
    }

    /// Rebuild a queue from a snapshot. The play order is kept as stored; if shuffle was on,
    /// turning it off later keeps that order (the original order is not stored).
    pub fn restore(snapshot: &QueueSnapshot, seed: u64) -> Queue {
        let base = Queue {
            repeat: snapshot.repeat,
            seed,
            ..Queue::default()
        };
        let loaded = base.play_now(&snapshot.tracks, snapshot.current.unwrap_or(0), seed);
        let entries = loaded.entries.clone();
        Queue {
            current: snapshot.current.filter(|i| *i < entries.len()),
            shuffle: snapshot.shuffle,
            natural: if snapshot.shuffle {
                entries.iter().map(|e| e.uid).collect()
            } else {
                vec![]
            },
            entries,
            ..loaded
        }
    }
}
