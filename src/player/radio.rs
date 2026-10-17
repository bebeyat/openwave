//! "Autoplay similar": when the queue ends, fetch songs like the last one and keep playing.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;

use super::controller::{
    advance, commit_queue, current_track, set_status, with_core, Status, QUEUE, STATUS,
};
use super::policy;
use super::queue::{Queue, Repeat};
use crate::backend::{api, Coded};
use crate::model::Track;
use crate::store;
use crate::ui::toast_error;

thread_local! {
    /// Similar songs fetched while the last song plays: (id of that song, the list). Only used
    /// when the queue then really ends on the same song.
    static AHEAD: RefCell<Option<(String, Vec<Track>)>> = const { RefCell::new(None) };
    /// Id of the song a background lookup is running for.
    static AHEAD_FOR: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// While the second-to-last or last song of the queue plays, look the next batch up so the end of the queue does
/// not wait for the catalog (measured about 0.8 to 1.1 s).
pub(super) fn prefetch_radio() {
    let queue = QUEUE.peek();
    if queue.upcoming().len() > 1 || queue.repeat() != Repeat::Off {
        return;
    }
    // The song the queue will end on: the last upcoming one, or the current one.
    let seed = queue
        .upcoming()
        .last()
        .map(|e| e.track.clone())
        .or_else(|| queue.current().map(|e| e.track.clone()));
    drop(queue);
    let Some(track) = seed else { return };
    if !store::settings().get().autoplay_similar || track.is_local() {
        return;
    }
    let id = track.id.to_string();
    let known = AHEAD.with(|a| a.borrow().as_ref().is_some_and(|(i, _)| *i == id))
        || AHEAD_FOR.with(|f| f.borrow().as_deref() == Some(id.as_str()));
    if known {
        return;
    }
    AHEAD_FOR.with(|f| *f.borrow_mut() = Some(id.clone()));
    spawn_forever(async move {
        if let Ok(list) = api::radio(&id).await {
            AHEAD.with(|a| *a.borrow_mut() = Some((id, list)));
        }
    });
}

fn take_ahead(id: &str) -> Option<Vec<Track>> {
    AHEAD.with(|a| {
        let mut slot = a.borrow_mut();
        if slot.as_ref().is_some_and(|(i, _)| i == id) {
            slot.take().map(|(_, list)| list)
        } else {
            None
        }
    })
}

/// Append songs similar to `seed` right away, while it plays. Skipped when the user has moved
/// on by the time the catalog answers, or when the queue already ended and the end-of-queue
/// lookup (`on_queue_end`) is filling it, so the two never race.
pub(super) fn queue_similar(seed: Track) {
    if seed.is_local() {
        return;
    }
    spawn_forever(async move {
        let Ok(list) = api::radio(&seed.id).await else {
            return;
        };
        let still_on_seed = current_track().is_some_and(|t| t.id == seed.id);
        if !still_on_seed || *STATUS.peek() == Status::FindingSimilar {
            return;
        }
        let fresh = fresh_tracks(&QUEUE.peek(), list);
        if fresh.is_empty() {
            return;
        }
        // Build the new queue first: the read borrow must end before `commit_queue` writes.
        let queue = QUEUE.peek().add_last(&fresh);
        commit_queue(queue);
    });
}

pub(super) fn on_queue_end() {
    set_status(Status::Paused);
    let settings = store::settings().get();
    let Some(track) = current_track() else { return };
    if !settings.autoplay_similar || track.is_local() {
        return;
    }
    if let Some(list) = take_ahead(&track.id) {
        apply_similar(Ok(list));
        return;
    }
    set_status(Status::FindingSimilar);
    let generation = with_core(|c| c.generation);
    spawn_forever(async move {
        let found = api::radio(&track.id).await;
        if with_core(|c| c.generation) != generation {
            return;
        }
        apply_similar(found);
    });
}

/// Queue the similar songs that are new and play the first; stay paused when there are none.
fn apply_similar(found: Result<Vec<Track>, crate::backend::ApiError>) {
    let fresh = match found {
        Ok(list) => fresh_tracks(&QUEUE.peek(), list),
        Err(e) => {
            toast_error(e.user_message());
            vec![]
        }
    };
    if fresh.is_empty() {
        set_status(Status::Paused);
        return;
    }
    let queue = QUEUE.peek().add_last(&fresh);
    commit_queue(queue);
    advance(false);
}

/// Radio results that are not already queued, capped to a batch.
pub(super) fn fresh_tracks(queue: &Queue, list: Vec<Track>) -> Vec<Track> {
    let known: HashSet<&str> = queue
        .entries()
        .iter()
        .map(|e| e.track.id.as_str())
        .collect();
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|t| !known.contains(t.id.as_str()) && seen.insert(t.id.to_string()))
        .take(policy::RADIO_BATCH)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn fresh_tracks_skips_queued_and_duplicates() {
        let q = Queue::new().add_last(&[track("a"), track("b")]);
        let list = vec![track("b"), track("c"), track("c"), track("d")];
        let ids: Vec<String> = fresh_tracks(&q, list)
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(ids, ["c", "d"]);
    }

    #[test]
    fn fresh_tracks_is_capped() {
        let list: Vec<Track> = (0..100).map(|i| track(&format!("t{i}"))).collect();
        assert_eq!(fresh_tracks(&Queue::new(), list).len(), policy::RADIO_BATCH);
    }
}
