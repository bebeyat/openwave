//! Dialog state (`Ctx`) and the async actions behind the buttons. Results of stale actions are
//! dropped through a generation counter, so cancelling or closing never lets an old request
//! change the screen. Matching itself cannot be aborted mid-flight, but nothing is saved by it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;

use super::clipboard::{read_text, ClipboardError};
use super::save;
use super::state::{check_link, find_link, transition, Event, Kind, LinkCheck, Step};
use crate::backend::api;
use crate::backend::error::{Coded, ImportError};
use crate::backend::import_flow::{ImportResult, MissingTrack, PreparedImport};
use crate::model::Track;

const POLL: Duration = Duration::from_millis(150);

/// What happened to the clipboard when the user asked to paste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    Found,
    Empty,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub message: String,
    pub retry: bool,
}

/// What the Result step shows. `playlist_id` is `None` for local files.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub playlist_id: Option<String>,
    pub tracks: Vec<Track>,
    pub missing: Vec<MissingTrack>,
    pub warning: Option<ImportError>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Ctx {
    pub step: Signal<Step>,
    pub kind: Signal<Kind>,
    pub input: Signal<String>,
    pub note: Signal<Option<Note>>,
    pub clip: Signal<Option<Kind>>,
    pub link_error: Signal<Option<ImportError>>,
    pub prepared: Signal<Option<PreparedImport>>,
    pub name: Signal<String>,
    pub progress: Signal<(usize, usize)>,
    pub outcome: Signal<Option<Outcome>>,
    pub failure: Signal<Option<Failure>>,
    pub confirming: Signal<bool>,
    generation: Signal<u64>,
}

pub fn use_ctx() -> Ctx {
    Ctx {
        step: use_signal(|| Step::Source),
        kind: use_signal(|| Kind::Spotify),
        input: use_signal(String::new),
        note: use_signal(|| None),
        clip: use_signal(|| None),
        link_error: use_signal(|| None),
        prepared: use_signal(|| None),
        name: use_signal(String::new),
        progress: use_signal(|| (0, 0)),
        outcome: use_signal(|| None),
        failure: use_signal(|| None),
        confirming: use_signal(|| false),
        generation: use_signal(|| 0),
    }
}

impl Ctx {
    fn go(self, event: Event) {
        let mut step = self.step;
        let next = transition(*step.peek(), event);
        step.set(next);
    }

    /// Start a new cancellable operation; any older one becomes stale.
    fn begin(self) -> u64 {
        let mut g = self.generation;
        let next = *g.peek() + 1;
        g.set(next);
        next
    }

    fn is_current(self, token: u64) -> bool {
        *self.generation.peek() == token
    }

    /// Forget all work (stale results are ignored from now on).
    pub fn cancel_work(self) {
        self.begin();
    }

    pub fn reset(self) {
        self.begin();
        let (mut step, mut kind, mut input, mut note) =
            (self.step, self.kind, self.input, self.note);
        let (mut clip, mut link_error, mut prepared) = (self.clip, self.link_error, self.prepared);
        let (mut name, mut progress, mut outcome) = (self.name, self.progress, self.outcome);
        let (mut failure, mut confirming) = (self.failure, self.confirming);
        step.set(Step::Source);
        kind.set(Kind::Spotify);
        input.set(String::new());
        note.set(None);
        clip.set(None);
        link_error.set(None);
        prepared.set(None);
        name.set(String::new());
        progress.set((0, 0));
        outcome.set(None);
        failure.set(None);
        confirming.set(false);
    }

    /// Look at the clipboard when the dialog opens (to hint which source has a link ready).
    pub fn peek_clipboard(self) {
        let mut clip = self.clip;
        spawn_forever(async move {
            let found = read_text().await.ok().and_then(|t| find_link(&t));
            clip.set(found.map(|(s, _)| Kind::of(&s)));
        });
    }

    pub fn pick(self, k: Kind) {
        let (mut kind, mut input, mut note, mut link_error) =
            (self.kind, self.input, self.note, self.link_error);
        kind.set(k);
        input.set(String::new());
        note.set(None);
        link_error.set(None);
        self.go(Event::Pick);
        spawn_forever(async move {
            let Ok(text) = read_text().await else { return };
            let Some((source, token)) = find_link(&text) else {
                return;
            };
            if Kind::of(&source) == k && input.peek().is_empty() {
                input.set(token);
                note.set(Some(Note::Found));
            }
        });
    }

    pub fn set_input(self, value: String) {
        let (mut input, mut note, mut link_error, mut kind) =
            (self.input, self.note, self.link_error, self.kind);
        if let LinkCheck::Valid(source) = check_link(&value) {
            kind.set(Kind::of(&source));
        }
        input.set(value);
        note.set(None);
        link_error.set(None);
    }

    pub fn paste(self) {
        spawn_forever(async move {
            let (mut input, mut note, mut link_error, mut kind) =
                (self.input, self.note, self.link_error, self.kind);
            link_error.set(None);
            match read_text().await {
                Ok(text) => match find_link(&text) {
                    Some((source, token)) => {
                        kind.set(Kind::of(&source));
                        input.set(token);
                        note.set(Some(Note::Found));
                    }
                    None => {
                        // Show the problem right away (instant validation of what was pasted).
                        input.set(text.trim().chars().take(300).collect());
                        note.set(None);
                    }
                },
                Err(ClipboardError::Empty) => note.set(Some(Note::Empty)),
                Err(ClipboardError::Unavailable) => note.set(Some(Note::Unavailable)),
            }
        });
    }

    pub fn back(self) {
        self.cancel_work();
        self.go(Event::Back);
    }

    /// Read the list (nothing is saved).
    pub fn submit(self) {
        let LinkCheck::Valid(source) = check_link(&self.input.peek()) else {
            return;
        };
        let (mut kind, mut link_error, mut prepared, mut name) =
            (self.kind, self.link_error, self.prepared, self.name);
        kind.set(Kind::of(&source));
        link_error.set(None);
        self.go(Event::Submit);
        let token = self.begin();
        spawn_forever(async move {
            let result = api::import_prepare(&source).await;
            if !self.is_current(token) {
                return;
            }
            match result {
                Ok(p) => {
                    name.set(p.preview.name.clone());
                    prepared.set(Some(p));
                    self.go(Event::PrepareOk);
                }
                Err(e) => {
                    link_error.set(Some(e));
                    self.go(Event::PrepareErr);
                }
            }
        });
    }

    /// Match the songs and save the playlist ("Kitaplığıma ekle").
    pub fn confirm(self) {
        let Some(prepared) = self.prepared.peek().clone() else {
            return;
        };
        let (mut progress, mut outcome, mut failure) = (self.progress, self.outcome, self.failure);
        progress.set((0, prepared.preview.count));
        failure.set(None);
        self.go(Event::Confirm);
        let token = self.begin();
        let typed_name = self.name.peek().clone();
        spawn_forever(async move {
            let result = run_with_progress(prepared, progress).await;
            if !self.is_current(token) {
                return;
            }
            match result.and_then(|r| save_result(r, &typed_name)) {
                Ok(done) => {
                    outcome.set(Some(done));
                    self.go(Event::RunOk);
                }
                Err(f) => {
                    failure.set(Some(f));
                    self.go(Event::RunErr);
                }
            }
        });
    }

    pub fn retry(self) {
        let mut s = self.step;
        if self.prepared.peek().is_some() {
            // Preview -> Matching again, exactly like pressing the confirm button.
            s.set(Step::Preview);
            self.confirm();
        } else {
            s.set(Step::Source);
        }
    }

    pub fn back_from_failure(self) {
        let mut s = self.step;
        s.set(if self.prepared.peek().is_some() {
            Step::Preview
        } else {
            Step::Source
        });
    }

    /// Open the native picker and remember the chosen files.
    pub fn pick_files(self) {
        let (mut outcome, mut failure) = (self.outcome, self.failure);
        self.go(Event::PickFiles);
        let token = self.begin();
        spawn_forever(async move {
            let picked = api::import_audio().await;
            if !self.is_current(token) {
                return;
            }
            match picked {
                Ok(tracks) if tracks.is_empty() => self.go(Event::FilesNone),
                Ok(tracks) => match save::save_local(&tracks) {
                    Ok(()) => {
                        outcome.set(Some(Outcome {
                            playlist_id: None,
                            tracks,
                            missing: vec![],
                            warning: None,
                        }));
                        self.go(Event::FilesDone);
                    }
                    Err(e) => {
                        failure.set(Some(Failure {
                            message: e.user_message(),
                            retry: false,
                        }));
                        self.go(Event::RunErr);
                    }
                },
                Err(e) => {
                    failure.set(Some(Failure {
                        message: e.user_message(),
                        retry: false,
                    }));
                    self.go(Event::RunErr);
                }
            }
        });
    }
}

async fn run_with_progress(
    prepared: PreparedImport,
    mut progress: Signal<(usize, usize)>,
) -> Result<ImportResult, Failure> {
    let seen = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
    let writer = seen.clone();
    let run = api::import_run(prepared, move |done, total| {
        writer.0.store(done, Ordering::Relaxed);
        writer.1.store(total, Ordering::Relaxed);
    });
    let mut run = std::pin::pin!(run);
    let result = loop {
        tokio::select! {
            r = &mut run => break r,
            _ = tokio::time::sleep(POLL) => {
                let (d, t) = (seen.0.load(Ordering::Relaxed), seen.1.load(Ordering::Relaxed));
                if t > 0 {
                    progress.set((d, t));
                }
            }
        }
    };
    result.map_err(|e| Failure {
        retry: matches!(e, ImportError::Offline | ImportError::Unreadable),
        message: e.user_message(),
    })
}

fn save_result(result: ImportResult, typed_name: &str) -> Result<Outcome, Failure> {
    let name = if typed_name.trim().is_empty() {
        result.name.as_str()
    } else {
        typed_name
    };
    let id = save::save_playlist(name, &result.tracks).map_err(|e| Failure {
        message: e.user_message(),
        retry: true,
    })?;
    Ok(Outcome {
        playlist_id: Some(id),
        tracks: result.tracks,
        missing: result.missing,
        warning: result.warning,
    })
}
