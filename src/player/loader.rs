//! Getting the current queue entry into the engine: asking the backend for a stream address, the
//! one retry, skipping a song that cannot be played, prefetching the next one and stopping.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use super::controller::{advance, set_status, with_core, Status, DURATION, POSITION, QUEUE};
use super::policy::{self, Failure};
use super::queue::Repeat;
use super::{controller::current_track, engine};
use crate::backend::{api, Coded};
use crate::i18n::t;
use crate::model::Track;
use crate::store;
use crate::ui::toast_error;

/// Load the current queue entry into the engine and (optionally) start it.
pub(super) fn start_current(start_secs: f64, autoplay: bool) {
    let Some(track) = current_track() else {
        stop_engine();
        return;
    };
    let generation = with_core(|c| {
        c.generation += 1;
        c.loaded = false;
        c.retried = false;
        c.recorded = false;
        c.resume_secs = 0.0;
        c.ended = false;
        c.generation
    });
    *POSITION.write() = start_secs;
    *DURATION.write() = track.duration_ms as f64 / 1000.0;
    set_status(Status::Loading);
    spawn_forever(load_task(generation, track, start_secs, autoplay, false));
}

async fn load_task(generation: u64, track: Track, start: f64, autoplay: bool, fresh: bool) {
    let quality = store::settings().get().quality();
    // The retry must not get the address that just failed from the backend's memory.
    let result = if fresh {
        match api::stream_fresh(&track.id, quality).await {
            // Last resort before giving up on the song: its SoundCloud copy.
            Err(e) if !track.is_local() && !track.id.starts_with("sc-") => {
                api::stream_soundcloud_copy(&track, quality)
                    .await
                    .map_err(|_| e)
            }
            other => other,
        }
    } else {
        api::stream(&track.id, quality).await
    };
    if with_core(|c| c.generation) != generation {
        return;
    }
    match result {
        Ok(stream) => {
            with_core(|c| c.loaded = true);
            engine::load(generation, &stream.url, start, autoplay);
            if !autoplay {
                set_status(Status::Paused);
            }
            prefetch_next();
        }
        Err(e) => on_failure(generation, &track, start, e.user_message()),
    }
}

pub(super) fn on_failure(generation: u64, track: &Track, start: f64, detail: String) {
    if with_core(|c| c.generation) != generation {
        return;
    }
    let (retried, streak) = with_core(|c| (c.retried, c.fail_streak));
    let step = policy::failure_step(retried, streak);
    if step == Failure::Retry {
        with_core(|c| c.retried = true);
        spawn_forever(load_task(generation, track.clone(), start, true, true));
        return;
    }
    with_core(|c| c.fail_streak += 1);
    let skipping = step == Failure::Skip && {
        let queue = QUEUE.peek();
        !queue.upcoming().is_empty() || (queue.repeat() == Repeat::All && queue.len() > 1)
    };
    let mut msg = format!("\"{}\": {detail}", track.title);
    if skipping {
        msg = format!("{msg} {}", t("player.skipping"));
    }
    toast_error(msg);
    // The source that just failed is dead: a "restart" of this song (repeat-one) must reload it.
    mark_source_dead();
    if skipping {
        // Not `auto`: repeat-one would reload the same dead song.
        advance(false);
    } else {
        set_status(Status::Paused);
    }
}

/// The engine no longer holds a usable source for the current song.
pub(super) fn mark_source_dead() {
    with_core(|c| c.loaded = false);
}

/// Upcoming songs whose stream address is looked up ahead of time.
pub(super) const PREFETCH_AHEAD: usize = 2;
/// Home candidates (the "continue" song, the first cards) that are looked up ahead of time.
pub const PREFETCH_HOME: usize = 3;

/// Online ids of the first `limit` tracks that are not local files.
pub(super) fn online_ids<'a>(tracks: impl Iterator<Item = &'a Track>, limit: usize) -> Vec<String> {
    tracks
        .filter(|t| !t.is_local())
        .take(limit)
        .map(|t| t.id.to_string())
        .collect()
}

pub(super) fn prefetch_next() {
    let queue = QUEUE.peek();
    if queue.repeat() == Repeat::One {
        return;
    }
    let near_end = queue.upcoming().len() <= 1;
    let ids = online_ids(queue.upcoming().iter().map(|e| &e.track), PREFETCH_AHEAD);
    drop(queue);
    if !ids.is_empty() {
        api::prefetch(&ids, store::settings().get().quality());
    }
    if near_end {
        super::radio::prefetch_radio();
    }
}

/// Look up stream addresses for songs the user is likely to play next (Home "continue" and the
/// first cards). Only the address is fetched, never audio; at most [`PREFETCH_HOME`] songs.
pub fn prefetch_tracks(tracks: &[Track]) {
    let ids = online_ids(tracks.iter(), PREFETCH_HOME);
    if !ids.is_empty() {
        api::prefetch(&ids, store::settings().get().quality());
    }
}

pub(super) fn stop_engine() {
    with_core(|c| {
        c.generation += 1;
        c.loaded = false;
        c.resume_secs = 0.0;
    });
    engine::stop();
    *POSITION.write() = 0.0;
    *DURATION.write() = 0.0;
    set_status(Status::Idle);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Track::default()
        }
    }

    #[test]
    fn online_ids_skip_local_files_and_respect_the_limit() {
        let tracks = [
            track("local-1"),
            track("aaaaaaaaaaa"),
            track("bbbbbbbbbbb"),
            track("ccccccccccc"),
        ];
        assert_eq!(
            online_ids(tracks.iter(), 2),
            vec!["aaaaaaaaaaa".to_owned(), "bbbbbbbbbbb".to_owned()]
        );
        assert!(online_ids(tracks[..1].iter(), 3).is_empty());
    }

    #[test]
    fn a_failed_source_is_marked_not_loaded() {
        with_core(|c| c.loaded = true);
        mark_source_dead();
        assert!(!with_core(|c| c.loaded));
    }
}
