//! Listen together (issue #43). The host's playback is sent to the room on the share relays
//! (see `backend::listen`); guests follow it: same song, same position, play and pause. Guests'
//! "add to queue" goes to the host's queue instead of their own. One session at a time.

mod dialog;
mod follow;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use dioxus::core::{spawn_forever, Task};
use dioxus::prelude::*;
use tokio::sync::broadcast;

use crate::backend::api;
use crate::backend::listen::{self, Connection, Msg, SigningKey};
use crate::i18n::t;
use crate::model::Track;
use crate::player::controller as ctl;
use crate::ui::{toast, toast_error};

pub use dialog::{ListenButton, ListenDialog};
use follow::{Follow, Publish};

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub code: String,
    pub host: bool,
    /// Guest: the host has been heard from.
    pub live: bool,
}

pub static SESSION: GlobalSignal<Option<Session>> = Signal::global(|| None);
pub static DIALOG_OPEN: GlobalSignal<bool> = Signal::global(|| false);

const TICK: Duration = Duration::from_millis(500);
/// Time the relay connections get to pass on the last message when a session ends.
const GOODBYE: Duration = Duration::from_millis(1500);

struct Live {
    key: SigningKey,
    code: String,
    send: broadcast::Sender<String>,
    task: Task,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

fn send(msg: &Msg) {
    LIVE.with(|l| {
        if let Some(l) = l.borrow().as_ref() {
            if let Ok(text) = listen::outgoing(&l.key, &l.code, msg) {
                let _ = l.send.send(text);
            }
        }
    });
}

/// Start hosting: a new room whose code comes from a fresh key.
pub fn start_host() {
    match listen::new_key() {
        Ok(key) => {
            let code = listen::code_of(&key);
            open(key, code, true);
        }
        Err(_) => toast_error(t("share.error.offline")),
    }
}

/// Join the room `code` as a guest.
pub fn join(code: String) {
    if SESSION.peek().as_ref().is_some_and(|s| s.code == code) {
        return;
    }
    match listen::new_key() {
        Ok(key) => open(key, code, false),
        Err(_) => toast_error(t("share.error.offline")),
    }
}

fn open(key: SigningKey, code: String, host: bool) {
    leave();
    let conn = match api::listen_connect(&code) {
        Ok(c) => c,
        Err(_) => return toast_error(t("share.error.offline")),
    };
    *SESSION.write() = Some(Session {
        code: code.clone(),
        host,
        live: false,
    });
    let send = conn.send.clone();
    let task = spawn_forever(run(conn, code.clone(), host));
    LIVE.with(|l| {
        *l.borrow_mut() = Some(Live {
            key,
            code,
            send,
            task,
        })
    });
}

/// End the session (host: tell the guests first).
pub fn leave() {
    let host = SESSION.peek().as_ref().is_some_and(|s| s.host);
    if host {
        send(&Msg::End);
    }
    if let Some(live) = LIVE.with(|l| l.borrow_mut().take()) {
        // The relay connections close with the task: give the goodbye a moment to go out.
        let wait = if host { GOODBYE } else { Duration::ZERO };
        spawn_forever(async move {
            tokio::time::sleep(wait).await;
            live.task.cancel();
        });
    }
    if SESSION.peek().is_some() {
        *SESSION.write() = None;
    }
}

/// Guest: send `tracks` to the host's queue. `false` when not a guest (add them locally).
pub fn suggest(tracks: &[Track]) -> bool {
    if SESSION.peek().as_ref().is_none_or(|s| s.host) {
        return false;
    }
    let catalog: Vec<Track> = tracks.iter().filter(|t| !t.is_local()).cloned().collect();
    if !catalog.is_empty() {
        send(&Msg::Add { t: catalog });
    }
    true
}

/// The "added to the queue" toast: a guest's songs went to the host's queue instead.
pub fn queued_text(key: &str) -> String {
    if SESSION.peek().as_ref().is_some_and(|s| !s.host) {
        t("listen.suggested")
    } else {
        t(key)
    }
}

async fn run(mut conn: Connection, code: String, host: bool) {
    let mut seen: VecDeque<String> = VecDeque::new();
    let mut publish = Publish::default();
    let mut follow = Follow::default();
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            event = conn.events.recv() => {
                let Some(event) = event else { return };
                let Some(incoming) = listen::read(&code, &event) else { continue };
                // Every relay passes the same event on.
                if seen.contains(&incoming.id) {
                    continue;
                }
                if seen.len() >= 64 {
                    seen.pop_front();
                }
                seen.push_back(incoming.id);
                match incoming.msg {
                    Msg::Add { t: tracks } if host && !tracks.is_empty() => {
                        ctl::add_last(&tracks);
                        toast(t("listen.added"));
                    }
                    Msg::State { seq, at, pos, play, q } if !host => {
                        mark_live();
                        follow.apply(seq, at, pos, play, &q);
                    }
                    Msg::End if !host => {
                        toast(t("listen.ended"));
                        LIVE.with(|l| l.borrow_mut().take());
                        *SESSION.write() = None;
                        return;
                    }
                    _ => {}
                }
            }
            _ = tick.tick(), if host => {
                if let Some(msg) = publish.tick() {
                    send(&msg);
                }
            }
        }
    }
}

fn mark_live() {
    let quiet = SESSION.peek().as_ref().is_some_and(|s| !s.live);
    if quiet {
        if let Some(s) = SESSION.write().as_mut() {
            s.live = true;
        }
    }
}

/// An invite link clicked outside the app (`openwave://j/<code>`).
pub fn open_invite(input: &str) -> bool {
    match listen::parse_join(input) {
        Some(code) => {
            join(code);
            *DIALOG_OPEN.write() = true;
            true
        }
        None => false,
    }
}
