//! Queue editing exposed to the UI: shuffle, repeat, remove, reorder (drag or menu) and clear.

use super::controller::{commit_queue, start_current, stop_engine, Status, QUEUE, STATUS};
use super::queue::{fresh_seed, Queue};
use dioxus::prelude::ReadableExt;

pub fn toggle_shuffle() {
    let q = QUEUE.peek().clone();
    let on = !q.shuffle();
    commit_queue(q.set_shuffle(on, fresh_seed()));
}

pub fn cycle_repeat() {
    let q = QUEUE.peek().cycle_repeat();
    commit_queue(q);
}

/// Remove an entry; removing the playing one moves on to the following entry. Removing the last
/// playing entry stops playback and leaves the new last entry as the (idle) current one.
pub fn remove_uid(uid: u64) {
    let before = QUEUE.peek().clone();
    let was_active = STATUS.peek().is_active();
    commit_queue(before.remove(uid));
    match removal_outcome(&before, uid) {
        Removal::PlayFollowing => start_current(0.0, was_active),
        Removal::Stop => stop_engine(),
        Removal::Keep => {}
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Removal {
    /// A background entry went away; playback is untouched.
    Keep,
    /// The playing entry went away and another one follows it.
    PlayFollowing,
    /// The playing entry was the last one: stop.
    Stop,
}

pub(super) fn removal_outcome(queue: &Queue, uid: u64) -> Removal {
    if queue.current_uid() != Some(uid) {
        return Removal::Keep;
    }
    match queue.current_index() {
        Some(i) if i + 1 < queue.len() => Removal::PlayFollowing,
        _ => Removal::Stop,
    }
}

/// Move an entry by `delta` places (keyboard-accessible reorder).
pub fn move_uid_by(uid: u64, delta: isize) {
    let q = QUEUE.peek().clone();
    let Some(from) = q.entries().iter().position(|e| e.uid == uid) else {
        return;
    };
    let to = from
        .saturating_add_signed(delta)
        .min(q.len().saturating_sub(1));
    commit_queue(q.move_entry(from, to));
}

/// Drop `uid` in front of `before` (0 = at the end), as reported by the drag script.
pub fn move_uid_before(uid: u64, before: u64) {
    let q = QUEUE.peek().clone();
    let Some(to) = drop_index(&q, uid, before) else {
        return;
    };
    let from = q.entries().iter().position(|e| e.uid == uid).unwrap_or(to);
    commit_queue(q.move_entry(from, to));
}

/// Final index of `uid` after it is dropped in front of `before` (0 = end of the list).
fn drop_index(queue: &Queue, uid: u64, before: u64) -> Option<usize> {
    let entries = queue.entries();
    let from = entries.iter().position(|e| e.uid == uid)?;
    if before == 0 {
        return Some(entries.len() - 1);
    }
    let b = entries.iter().position(|e| e.uid == before)?;
    Some(if from < b { b - 1 } else { b })
}

/// "Sırayı temizle": keeps only the playing song (everything when nothing plays).
pub fn clear_queue() {
    let q = QUEUE.peek().clone();
    if q.current().is_some() && *STATUS.peek() != Status::Idle {
        commit_queue(q.clear_upcoming());
    } else {
        commit_queue(q.clear());
        stop_engine();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Track;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn removing_the_playing_entry_follows_or_stops() {
        let q = Queue::new().play_now(&[track("a"), track("b"), track("c")], 1, 1);
        let uid = |i: usize| q.entries()[i].uid;
        assert_eq!(removal_outcome(&q, uid(0)), Removal::Keep);
        assert_eq!(removal_outcome(&q, uid(2)), Removal::Keep);
        assert_eq!(removal_outcome(&q, uid(1)), Removal::PlayFollowing);
        let last = q.play_uid(uid(2)).queue;
        assert_eq!(removal_outcome(&last, last.entries()[2].uid), Removal::Stop);
        let only = Queue::new().play_now(&[track("a")], 0, 1);
        assert_eq!(removal_outcome(&only, only.entries()[0].uid), Removal::Stop);
        assert_eq!(removal_outcome(&Queue::new(), 7), Removal::Keep);
    }

    #[test]
    fn drop_index_accounts_for_removal() {
        let q = Queue::new().add_last(&[track("a"), track("b"), track("c"), track("d")]);
        let uids: Vec<u64> = q.entries().iter().map(|e| e.uid).collect();
        // a dropped before d: lands at index 2 (after removing a, d is at 2)
        assert_eq!(drop_index(&q, uids[0], uids[3]), Some(2));
        // d dropped before b: lands at index 1
        assert_eq!(drop_index(&q, uids[3], uids[1]), Some(1));
        // b dropped at the end
        assert_eq!(drop_index(&q, uids[1], 0), Some(3));
        assert_eq!(drop_index(&q, 999, 0), None);
        assert_eq!(drop_index(&q, uids[0], 999), None);
    }
}
