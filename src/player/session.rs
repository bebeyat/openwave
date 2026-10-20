//! Saving and restoring the play session: debounced queue snapshots, resume at start-up and the
//! play history entry. Everything is written through the library store.

use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use super::controller::{self, with_core, DURATION, POSITION, QUEUE};
use super::policy;
use super::queue::{fresh_seed, Queue};
use crate::features::tracks::state::update_library;
use crate::store::{self, library};

/// Write the queue and position to disk after a short quiet time (restarts the timer on each call).
pub fn schedule_persist() {
    let token = with_core(|c| {
        c.persist_token += 1;
        c.persist_token
    });
    spawn_forever(async move {
        tokio::time::sleep(Duration::from_millis(policy::PERSIST_DEBOUNCE_MS)).await;
        if with_core(|c| c.persist_token) == token {
            persist_now();
        }
    });
}

/// Write the queue and position right now.
pub fn persist_now() {
    with_core(|c| {
        c.persist_token += 1;
        c.last_persist_ms = library::now_ms();
    });
    let queue = QUEUE.peek();
    let snapshot = (!queue.is_empty()).then(|| {
        let duration_ms = queue.current().map_or(0, |e| e.track.duration_ms);
        queue.snapshot(policy::snapshot_position_ms(*POSITION.peek(), duration_ms))
    });
    drop(queue);
    update_library(|lib| lib.set_last_queue(snapshot));
}

/// Save the queue and write every store now. The window loop ends with `process::exit`, so
/// nothing runs after the window closes: call this before it does.
pub fn save_before_exit() {
    persist_now();
    store::flush_all();
}

/// Called with every time update from the engine: saves the position now and then.
pub fn position_tick() {
    let now = library::now_ms();
    if policy::persist_due(now, with_core(|c| c.last_persist_ms)) {
        persist_now();
    }
}

/// A Next press this early in a song counts as a skip for the recommendations.
pub const SKIP_WITHIN_SECS: f64 = 12.0;

/// Called when the user presses Next at `position` seconds.
pub fn record_skip_if_early(position: f64) {
    if position >= SKIP_WITHIN_SECS || !with_core(|c| c.recorded) {
        return;
    }
    if let Some(track) = controller::current_track() {
        update_library(|lib| lib.record_skip(&track.id));
    }
}

/// Put the song into the play history the first time it actually starts.
pub fn record_play_once() {
    if with_core(|c| std::mem::replace(&mut c.recorded, true)) {
        return;
    }
    if let Some(track) = controller::current_track() {
        let now = library::now_ms();
        update_library(|lib| lib.record_play(&track, now));
    }
}

/// Restore the last queue (paused, nothing loaded yet) when "resume queue" is on.
pub fn restore() {
    if !QUEUE.peek().is_empty() || !store::settings().get().resume_queue {
        return;
    }
    let lib = store::library().get();
    let Some(snapshot) = lib.last_queue.as_ref() else {
        return;
    };
    if snapshot.tracks.is_empty() || snapshot.current.is_none() {
        return;
    }
    let queue = Queue::restore(snapshot, fresh_seed());
    let secs = snapshot.position_ms as f64 / 1000.0;
    let duration = queue
        .current()
        .map_or(0.0, |e| e.track.duration_ms as f64 / 1000.0);
    controller::commit_queue(queue);
    with_core(|c| c.resume_secs = secs);
    *POSITION.write() = secs;
    *DURATION.write() = duration;
}
