//! Queue editing: reorder, remove, clear, repeat and shuffle switches.
use super::{Queue, Repeat};

impl Queue {
    /// Move the entry at index `from` to index `to` (drag-reorder). The current entry follows.
    pub fn move_entry(&self, from: usize, to: usize) -> Queue {
        if from >= self.len() || to >= self.len() || from == to {
            return self.clone();
        }
        let mut entries = self.entries.clone();
        let item = entries.remove(from);
        entries.insert(to, item);
        let current = self
            .current_uid()
            .and_then(|u| entries.iter().position(|e| e.uid == u));
        Queue {
            entries,
            current,
            ..self.clone()
        }
    }

    /// Remove one entry. If it was the current one, the following entry becomes current (the
    /// caller decides whether to start it). If it was the last entry, the new last entry stays
    /// current: the queue is then in its "ended" state (Next stops, or wraps with repeat-all) and
    /// never falls back to "nothing is current" while entries remain. Only an empty queue has no
    /// current entry.
    pub fn remove(&self, uid: u64) -> Queue {
        let Some(idx) = self.entries.iter().position(|e| e.uid == uid) else {
            return self.clone();
        };
        let entries: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.uid != uid)
            .cloned()
            .collect();
        let current = match self.current {
            Some(c) if idx < c => Some(c - 1),
            Some(c) if idx == c => entries.len().checked_sub(1).map(|last| idx.min(last)),
            other => other,
        };
        Queue {
            natural: self.natural.iter().copied().filter(|u| *u != uid).collect(),
            history: self.history.iter().copied().filter(|u| *u != uid).collect(),
            entries,
            current,
            ..self.clone()
        }
    }

    /// Empty the queue (repeat/shuffle settings stay).
    pub fn clear(&self) -> Queue {
        Queue {
            repeat: self.repeat,
            shuffle: self.shuffle,
            seed: self.seed,
            next_uid: self.next_uid,
            ..Queue::default()
        }
    }

    /// Keep only the playing entry ("Sırayı temizle" while something plays).
    pub fn clear_upcoming(&self) -> Queue {
        match self.current() {
            None => self.clear(),
            Some(cur) => Queue {
                entries: vec![cur.clone()],
                current: Some(0),
                natural: if self.shuffle { vec![cur.uid] } else { vec![] },
                history: vec![],
                ..self.clone()
            },
        }
    }

    pub fn set_repeat(&self, repeat: Repeat) -> Queue {
        Queue {
            repeat,
            ..self.clone()
        }
    }

    /// Off -> All -> One -> Off.
    pub fn cycle_repeat(&self) -> Queue {
        let next = match self.repeat {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
        self.set_repeat(next)
    }

    /// Turn shuffle on (current entry first, the rest shuffled with `seed`) or off (original
    /// order restored, current entry kept).
    pub fn set_shuffle(&self, on: bool, seed: u64) -> Queue {
        if on == self.shuffle {
            return self.clone();
        }
        if on {
            return Queue {
                shuffle: true,
                seed,
                ..self.clone()
            }
            .reshuffled_around_current();
        }
        let mut entries = self.entries.clone();
        let rank = |uid: u64| {
            self.natural
                .iter()
                .position(|n| *n == uid)
                .unwrap_or(usize::MAX)
        };
        entries.sort_by_key(|e| rank(e.uid));
        let current = self
            .current_uid()
            .and_then(|u| entries.iter().position(|e| e.uid == u));
        Queue {
            entries,
            current,
            shuffle: false,
            natural: vec![],
            ..self.clone()
        }
    }
}
