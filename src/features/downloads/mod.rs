//! Offline listening: a download queue, the list of downloaded tracks and helpers for the UI.
//!
//! Tracks are downloaded one at a time by a task that outlives any page (`spawn_forever`). A
//! downloaded track plays from disk (see `backend::media::download`), so it also works offline.

mod button;

pub use button::DownloadAllButton;

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::i18n::{t, t_count};
use crate::model::Track;
use crate::store;
use crate::ui::{toast, toast_error};

/// Downloaded tracks, newest first.
pub static DOWNLOADS: GlobalSignal<Vec<api::Downloaded>> = Signal::global(Vec::new);
/// Waiting to be downloaded, in order.
pub static PENDING: GlobalSignal<VecDeque<Track>> = Signal::global(VecDeque::new);
/// Set by [`cancel_all`] so the runner reports a cancel instead of an error.
static CANCELLED: AtomicBool = AtomicBool::new(false);
/// Set while the queue runner exists; claimed synchronously so two runners never start.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// The track being downloaded right now.
pub static ACTIVE: GlobalSignal<Option<String>> = Signal::global(|| None);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    None,
    Queued,
    Downloading,
    Done,
}

/// Only catalog tracks can be downloaded (local files already are on disk).
pub fn downloadable(t: &Track) -> bool {
    !t.is_local() && crate::backend::media::video_key(&t.id)
}

pub fn state(id: &str) -> State {
    if DOWNLOADS.read().iter().any(|d| d.track.id == id) {
        State::Done
    } else if ACTIVE.read().as_deref() == Some(id) {
        State::Downloading
    } else if PENDING.read().iter().any(|t| t.id == id) {
        State::Queued
    } else {
        State::None
    }
}

/// Ids of downloaded tracks (for marking rows).
pub fn downloaded_ids() -> HashSet<String> {
    DOWNLOADS
        .read()
        .iter()
        .map(|d| d.track.id.to_string())
        .collect()
}

/// Total size of the downloads, in bytes.
pub fn total_bytes() -> u64 {
    DOWNLOADS.read().iter().map(|d| d.bytes).sum()
}

/// Read the downloads folder (once at startup and after every change).
pub fn refresh() {
    spawn_forever(async {
        if let Ok(list) = api::downloads().await {
            *DOWNLOADS.write() = list;
        }
    });
}

/// The tracks of `tracks` that still need downloading: catalog tracks, not done, not queued.
pub fn to_download(
    tracks: &[Track],
    done: &HashSet<String>,
    queued: &HashSet<String>,
) -> Vec<Track> {
    let mut seen = HashSet::new();
    tracks
        .iter()
        .filter(|t| downloadable(t))
        .filter(|t| !done.contains(t.id.as_str()) && !queued.contains(t.id.as_str()))
        .filter(|t| seen.insert(t.id.to_string()))
        .cloned()
        .collect()
}

/// Queue `tracks` for download (already downloaded or queued ones are skipped).
pub fn enqueue(tracks: &[Track]) {
    let done = downloaded_ids();
    let mut queued: HashSet<String> = PENDING.peek().iter().map(|t| t.id.to_string()).collect();
    if let Some(a) = ACTIVE.peek().clone() {
        queued.insert(a);
    }
    let fresh = to_download(tracks, &done, &queued);
    if fresh.is_empty() {
        toast(t("downloads.toast.nothing"));
        return;
    }
    toast(t_count("downloads.toast.queued", fresh.len()));
    CANCELLED.store(false, Ordering::SeqCst);
    PENDING.write().extend(fresh);
    if !RUNNING.swap(true, Ordering::SeqCst) {
        spawn_forever(run_queue());
    }
}

async fn run_queue() {
    let (mut ok, mut failed) = (0usize, None);
    'runner: loop {
        drain(&mut ok, &mut failed).await;
        RUNNING.store(false, Ordering::SeqCst);
        // Something queued between the last pop and the store above: keep going.
        if PENDING.peek().is_empty() || RUNNING.swap(true, Ordering::SeqCst) {
            break 'runner;
        }
    }
    *ACTIVE.write() = None;
    if CANCELLED.swap(false, Ordering::SeqCst) {
        toast(t("downloads.toast.cancelled"));
        return;
    }
    match failed {
        Some(msg) => toast_error(format!("{} {msg}", t_count("downloads.toast.done", ok))),
        None if ok > 0 => toast(t_count("downloads.toast.done", ok)),
        None => {}
    }
}

/// Download everything in [`PENDING`], one track at a time.
async fn drain(ok: &mut usize, failed: &mut Option<String>) {
    loop {
        let Some(track) = PENDING.write().pop_front() else {
            break;
        };
        *ACTIVE.write() = Some(track.id.to_string());
        let quality = store::settings().get().quality();
        match api::download_track(&track, quality).await {
            Ok(_) => {
                *ok += 1;
                if let Ok(list) = api::downloads().await {
                    *DOWNLOADS.write() = list;
                }
            }
            Err(e) => *failed = Some(e.user_message()),
        }
    }
}

/// Delete one download.
pub fn delete(id: &str) {
    let id = id.to_owned();
    spawn_forever(async move {
        match api::remove_download(&id).await {
            Ok(()) => {
                DOWNLOADS.write().retain(|d| d.track.id != id);
                toast(t("downloads.toast.removed"));
            }
            Err(e) => toast_error(e.user_message()),
        }
    });
}

/// Stop the queue after the current track.
pub fn cancel_pending() {
    PENDING.write().clear();
}

/// Cancel everything: the queue and the track being downloaded right now.
pub fn cancel_all() {
    CANCELLED.store(true, Ordering::SeqCst);
    PENDING.write().clear();
    crate::backend::media::cancel_downloads();
}

/// Progress of one list: (downloaded, downloadable, any of its tracks queued or downloading).
pub fn list_progress(tracks: &[Track]) -> (usize, usize, bool) {
    let done = downloaded_ids();
    let pending: HashSet<String> = PENDING.read().iter().map(|t| t.id.to_string()).collect();
    let active = ACTIVE.read().clone();
    let mut seen = HashSet::new();
    let list: Vec<&Track> = tracks
        .iter()
        .filter(|t| downloadable(t) && seen.insert(t.id.to_string()))
        .collect();
    let finished = list.iter().filter(|t| done.contains(t.id.as_str())).count();
    let busy = list
        .iter()
        .any(|t| pending.contains(t.id.as_str()) || active.as_deref() == Some(t.id.as_str()));
    (finished, list.len(), busy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(id: &str) -> Track {
        Track {
            id: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn only_new_catalog_tracks_are_queued_once() {
        let tracks = vec![
            tr("aaaaaaaaaaa"),
            tr("bbbbbbbbbbb"),
            tr("aaaaaaaaaaa"),
            tr("local-123"),
            tr("ccccccccccc"),
            tr("not-a-video-id"),
        ];
        let done: HashSet<String> = ["bbbbbbbbbbb".to_string()].into();
        let queued: HashSet<String> = ["ccccccccccc".to_string()].into();
        let ids: Vec<String> = to_download(&tracks, &done, &queued)
            .iter()
            .map(|t| t.id.to_string())
            .collect();
        assert_eq!(ids, ["aaaaaaaaaaa"]);
    }
}
