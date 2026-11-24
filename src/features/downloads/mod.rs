//! Offline listening: a download queue, the list of downloaded tracks and helpers for the UI.
//!
//! Tracks are downloaded one at a time by a task that outlives any page (`spawn_forever`). A
//! downloaded track plays from disk (see `backend::media::download`), so it also works offline.

mod button;
mod live;
mod queue;

pub use button::DownloadAllButton;
pub use queue::{QueuePanel, SidebarProgress};

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
pub static ACTIVE: GlobalSignal<Option<Track>> = Signal::global(|| None);
/// Tracks whose download failed, with the reason, oldest first.
pub static FAILED: GlobalSignal<Vec<(Track, String)>> = Signal::global(Vec::new);
/// Tracks finished (downloaded, failed or skipped) since the queue last started; with the
/// tracks still waiting this gives the progress of the whole run.
pub static RUN_DONE: GlobalSignal<usize> = Signal::global(|| 0);
/// The queue is paused: nothing new starts until [`resume`].
pub static PAUSED: GlobalSignal<bool> = Signal::global(|| false);
/// Set by [`cancel_active`] so the runner drops that one track instead of reporting it failed.
static SKIP_ACTIVE: AtomicBool = AtomicBool::new(false);

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
    } else if ACTIVE.read().as_ref().is_some_and(|t| t.id == id) {
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
    if let Some(a) = ACTIVE.peek().as_ref() {
        queued.insert(a.id.to_string());
    }
    let fresh = to_download(tracks, &done, &queued);
    if fresh.is_empty() {
        toast(t("downloads.toast.nothing"));
        return;
    }
    toast(t_count("downloads.toast.queued", fresh.len()));
    FAILED
        .write()
        .retain(|(f, _)| !fresh.iter().any(|t| t.id == f.id));
    CANCELLED.store(false, Ordering::SeqCst);
    PENDING.write().extend(fresh);
    start_runner();
}

/// Start the queue runner unless it runs already or the queue is paused.
fn start_runner() {
    if !*PAUSED.peek() && !RUNNING.swap(true, Ordering::SeqCst) {
        live::start(|| RUNNING.load(Ordering::SeqCst));
        spawn_forever(run_queue());
    }
}

async fn run_queue() {
    let (mut ok, mut failed) = (0usize, None);
    'runner: loop {
        drain(&mut ok, &mut failed).await;
        RUNNING.store(false, Ordering::SeqCst);
        // Something queued between the last pop and the store above: keep going.
        if PENDING.peek().is_empty() || *PAUSED.peek() || RUNNING.swap(true, Ordering::SeqCst) {
            break 'runner;
        }
    }
    *ACTIVE.write() = None;
    if *PAUSED.peek() {
        return;
    }
    *RUN_DONE.write() = 0;
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
        if *PAUSED.peek() {
            break;
        }
        let Some(track) = PENDING.write().pop_front() else {
            break;
        };
        *ACTIVE.write() = Some(track.clone());
        SKIP_ACTIVE.store(false, Ordering::SeqCst);
        let quality = store::settings().get().quality();
        let result = api::download_track(&track, quality).await;
        if result.is_err() && *PAUSED.peek() {
            // Stopped by [`pause`]: it starts over first when the queue resumes.
            PENDING.write().push_front(track);
            break;
        }
        *RUN_DONE.write() += 1;
        match result {
            Ok(_) => {
                *ok += 1;
                if let Ok(list) = api::downloads().await {
                    *DOWNLOADS.write() = list;
                }
            }
            Err(_) if CANCELLED.load(Ordering::SeqCst) || SKIP_ACTIVE.load(Ordering::SeqCst) => {}
            Err(e) => {
                let msg = e.user_message();
                FAILED.write().push((track, msg.clone()));
                *failed = Some(msg);
            }
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

/// Copy downloads into a folder the user picks (tagged `.m4a` files).
pub fn export(ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    spawn_forever(async move {
        let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
            return;
        };
        let dir = folder.path().to_path_buf();
        let (mut ok, mut failed) = (0usize, None);
        for id in &ids {
            match api::export_download(id, dir.clone()).await {
                Ok(_) => ok += 1,
                Err(e) => failed = Some(e.user_message()),
            }
        }
        match failed {
            Some(msg) => toast_error(format!("{} {msg}", t_count("downloads.toast.exported", ok))),
            None => toast(t_count("downloads.toast.exported", ok)),
        }
    });
}

/// Take one waiting track out of the queue.
pub fn remove_pending(id: &str) {
    PENDING.write().retain(|t| t.id != id);
}

/// Stop the track being downloaded right now; the queue goes on with the next one.
pub fn cancel_active() {
    SKIP_ACTIVE.store(true, Ordering::SeqCst);
    crate::backend::media::cancel_downloads();
}

/// Pause the queue; the song downloading now stops and goes back to the front of the queue.
pub fn pause() {
    *PAUSED.write() = true;
    if ACTIVE.peek().is_some() {
        crate::backend::media::cancel_downloads();
    }
}

/// Go on with the queue after [`pause`].
pub fn resume() {
    *PAUSED.write() = false;
    if PENDING.peek().is_empty() {
        *RUN_DONE.write() = 0;
    }
    start_runner();
}

/// Queue the failed downloads again.
pub fn retry_failed() {
    let tracks: Vec<Track> = FAILED.peek().iter().map(|(t, _)| t.clone()).collect();
    enqueue(&tracks);
}

/// Forget one failed download.
pub fn dismiss_failed(id: &str) {
    FAILED.write().retain(|(t, _)| t.id != id);
}

/// Cancel everything: the queue and the track being downloaded right now.
pub fn cancel_all() {
    CANCELLED.store(true, Ordering::SeqCst);
    PENDING.write().clear();
    *PAUSED.write() = false;
    if !RUNNING.load(Ordering::SeqCst) {
        *RUN_DONE.write() = 0;
    }
    crate::backend::media::cancel_downloads();
}

/// Progress of one list: (downloaded, downloadable, any of its tracks queued or downloading).
pub fn list_progress(tracks: &[Track]) -> (usize, usize, bool) {
    let done = downloaded_ids();
    let pending: HashSet<String> = PENDING.read().iter().map(|t| t.id.to_string()).collect();
    let active = ACTIVE.read().as_ref().map(|t| t.id.to_string());
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
