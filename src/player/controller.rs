//! The single entry point screens use to start or queue music, and the owner of playback state.
//!
//! The queue state machine (`queue.rs`) decides *what* plays; this module talks to the audio
//! engine (`engine.rs`), reacts to its events and publishes state as global signals. Signatures
//! of `play_now`, `add_next`, `add_last`, `toggle_pause` and `NOW_PLAYING` are a stable contract.

use std::cell::RefCell;

use dioxus::prelude::*;

use super::engine;
use super::policy;
use super::queue::{fresh_seed, Action, Queue};
use super::radio::on_queue_end;
use super::{media_keys, session};
use crate::model::{Id, Track};

pub use super::events::handle_event;
pub use super::loader::prefetch_tracks;
pub(super) use super::loader::{on_failure, prefetch_next, start_current, stop_engine};
pub use super::queue_edit::{
    clear_queue, cycle_repeat, move_uid_before, move_uid_by, remove_uid, toggle_shuffle,
};

/// What the rest of the UI needs to know about playback (highlighting the playing row, etc.).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NowPlaying {
    pub track_id: Option<Id>,
    pub playing: bool,
}

pub static NOW_PLAYING: GlobalSignal<NowPlaying> = Signal::global(NowPlaying::default);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// Nothing is loaded in the engine (empty queue, or a restored queue not started yet).
    #[default]
    Idle,
    /// Asking the backend for the stream address.
    Loading,
    Playing,
    /// Playing but waiting for data.
    Buffering,
    Paused,
    /// The queue ended and "autoplay similar" is looking for more songs.
    FindingSimilar,
}

impl Status {
    /// Audio is (or should be) coming out of the speakers.
    pub fn is_active(self) -> bool {
        matches!(self, Status::Playing | Status::Buffering)
    }
}

pub static QUEUE: GlobalSignal<Queue> = Signal::global(Queue::new);
pub static STATUS: GlobalSignal<Status> = Signal::global(Status::default);
/// Playing position in seconds (updated about four times a second while playing only).
pub static POSITION: GlobalSignal<f64> = Signal::global(|| 0.0);
/// Length in seconds (from the engine once known, else from the track metadata).
pub static DURATION: GlobalSignal<f64> = Signal::global(|| 0.0);
pub static VOLUME: GlobalSignal<f64> = Signal::global(|| 0.8);
pub static MUTED: GlobalSignal<bool> = Signal::global(|| false);

/// Non-reactive bookkeeping (the UI thread is the only user).
#[derive(Default)]
pub(super) struct Core {
    /// Bumped for every load; stale engine events and async results are dropped.
    pub generation: u64,
    /// The engine holds the current track's source.
    pub loaded: bool,
    pub retried: bool,
    pub fail_streak: u32,
    /// Position to start from when a restored (not yet loaded) queue is played.
    pub resume_secs: f64,
    pub recorded: bool,
    pub last_persist_ms: u64,
    pub persist_token: u64,
    /// The last song finished and nothing followed; cleared once audio plays again.
    pub ended: bool,
    /// Bumped by every `play_now`, so a slower "look up, then play" never overrides a newer play.
    pub play_request: u64,
}

thread_local! {
    static CORE: RefCell<Core> = RefCell::new(Core::default());
}

pub(super) fn with_core<R>(f: impl FnOnce(&mut Core) -> R) -> R {
    CORE.with(|c| f(&mut c.borrow_mut()))
}

// ---- reading state -----------------------------------------------------------------------

pub fn current_track() -> Option<Track> {
    QUEUE.peek().current().map(|e| e.track.clone())
}

/// Duration to show: the engine's, else the catalog's.
pub fn display_duration() -> f64 {
    let d = *DURATION.peek();
    if d > 0.0 {
        d
    } else {
        current_track().map_or(0.0, |t| t.duration_ms as f64 / 1000.0)
    }
}

// ---- state plumbing ----------------------------------------------------------------------

pub(super) fn set_status(status: Status) {
    if status == Status::Playing {
        with_core(|c| c.ended = false);
    }
    if *STATUS.peek() != status {
        *STATUS.write() = status;
    }
    sync_now_playing();
    media_keys::update_playback(status, *POSITION.peek());
}

fn sync_now_playing() {
    let next = NowPlaying {
        track_id: QUEUE.peek().current().map(|e| e.track.id.clone()),
        playing: STATUS.peek().is_active(),
    };
    if *NOW_PLAYING.peek() != next {
        *NOW_PLAYING.write() = next;
    }
}

/// Store a new queue value, keep dependants in sync and schedule saving it.
pub(super) fn commit_queue(next: Queue) {
    let changed_track = next.current().map(|e| e.track.id.clone())
        != QUEUE.peek().current().map(|e| e.track.id.clone());
    *QUEUE.write() = next;
    sync_now_playing();
    if changed_track {
        media_keys::update_track(current_track().as_ref());
    }
    session::schedule_persist();
}

// ---- public transport (stable contract) --------------------------------------------------

/// Replace the queue with `tracks` and start playing at `start`.
pub fn play_now(tracks: &[Track], start: usize) {
    with_core(|c| c.play_request += 1);
    let next = QUEUE.peek().play_now(tracks, start, fresh_seed());
    let empty = next.is_empty();
    commit_queue(next);
    if empty {
        stop_engine();
    } else {
        start_current(0.0, true);
    }
}

/// Ticket for a "look the songs up, then play" request; check it with [`is_latest_play`]
/// before playing the result.
pub fn play_request() -> u64 {
    with_core(|c| c.play_request)
}

/// No play started since `ticket` was taken.
pub fn is_latest_play(ticket: u64) -> bool {
    with_core(|c| c.play_request == ticket)
}

/// Play `track` alone and queue songs similar to it behind it (from the catalog radio), so
/// "next" and the end of the song continue with something like it. Used for songs started from
/// search, where the other results are unrelated.
pub fn play_with_similar(track: &Track) {
    play_now(std::slice::from_ref(track), 0);
    super::radio::queue_similar(track.clone());
}

/// Insert after the current track.
pub fn add_next(tracks: &[Track]) {
    add_tracks(tracks, true);
}

/// Append to the end of the queue.
pub fn add_last(tracks: &[Track]) {
    add_tracks(tracks, false);
}

fn add_tracks(tracks: &[Track], next: bool) {
    if tracks.is_empty() {
        return;
    }
    let was_empty = QUEUE.peek().is_empty();
    // After the queue ended, Play would restart the finished song: move on to the added one.
    let ended = with_core(|c| c.ended) && !STATUS.peek().is_active();
    let base = QUEUE.peek().clone();
    let updated = if next {
        base.add_next(tracks)
    } else {
        base.add_last(tracks)
    };
    commit_queue(updated);
    if was_empty {
        let step = QUEUE.peek().next(false);
        commit_queue(step.queue);
        start_current(0.0, true);
    } else if ended {
        advance(false);
    } else {
        prefetch_next();
    }
}

/// Toggle play/pause of the current track.
pub fn toggle_pause() {
    if current_track().is_none() {
        return;
    }
    let (loaded, resume) = with_core(|c| (c.loaded, c.resume_secs));
    let status = *STATUS.peek();
    let position = *POSITION.peek();
    match status {
        Status::Playing | Status::Buffering => pause(),
        Status::Paused if loaded => engine::play(),
        Status::Paused | Status::Idle => start_current(resume.max(position), true),
        Status::Loading | Status::FindingSimilar => {}
    }
}

pub fn pause() {
    if STATUS.peek().is_active() {
        engine::pause();
        set_status(Status::Paused);
        session::persist_now();
    }
}

/// Resume without toggling (media key "Play").
pub fn resume() {
    if !STATUS.peek().is_active() {
        toggle_pause();
    }
}

// ---- navigation --------------------------------------------------------------------------

/// Next song (the user pressed Next, or the song ended by itself when `auto`).
pub fn advance(auto: bool) {
    let step = QUEUE.peek().next(auto);
    commit_queue(step.queue);
    match step.action {
        Action::Play => start_current(0.0, true),
        Action::Restart => restart(),
        Action::Stop => on_queue_end(),
    }
}

pub fn next() {
    advance(false);
}

pub fn previous() {
    if policy::previous_restarts(*POSITION.peek()) {
        restart();
        return;
    }
    let step = QUEUE.peek().previous();
    commit_queue(step.queue);
    match step.action {
        Action::Play => start_current(0.0, true),
        Action::Restart | Action::Stop => restart(),
    }
}

fn restart() {
    match policy::restart_plan(with_core(|c| c.loaded), current_track().is_some()) {
        policy::Restart::Seek => {
            seek_to(0.0);
            engine::play();
        }
        policy::Restart::Reload => start_current(0.0, true),
        policy::Restart::Nothing => {}
    }
}

/// Jump to a queue entry (click in the queue list).
pub fn play_uid(uid: u64) {
    let step = QUEUE.peek().play_uid(uid);
    commit_queue(step.queue);
    match step.action {
        Action::Play => start_current(0.0, true),
        Action::Restart => restart(),
        Action::Stop => {}
    }
}

// ---- position, volume --------------------------------------------------------------------

pub fn seek_to(seconds: f64) {
    let target = engine::seek_target(seconds, 0.0, display_duration());
    *POSITION.write() = target;
    if with_core(|c| c.loaded) {
        engine::seek(target);
    } else {
        with_core(|c| c.resume_secs = target);
    }
}

pub fn seek_by(delta: f64) {
    let target = engine::seek_target(*POSITION.peek(), delta, display_duration());
    seek_to(target);
}

pub fn set_volume(volume: f64) {
    let v = engine::clamp_volume(volume);
    *VOLUME.write() = v;
    if v > 0.0 && *MUTED.peek() {
        set_muted(false);
    }
    engine::set_volume(v);
}

pub fn volume_by(delta: f64) {
    // Read into a local first: a `peek()` guard alive during the call would block the write.
    let current = *VOLUME.peek();
    set_volume(current + delta);
}

pub fn set_muted(muted: bool) {
    *MUTED.write() = muted;
    engine::set_muted(muted);
}

pub fn toggle_mute() {
    let muted = *MUTED.peek();
    set_muted(!muted);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_playing_and_buffering_are_active() {
        assert!(Status::Playing.is_active());
        assert!(Status::Buffering.is_active());
        assert!(!Status::Paused.is_active());
        assert!(!Status::Idle.is_active());
        assert!(!Status::Loading.is_active());
    }
}
