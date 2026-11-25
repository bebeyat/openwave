//! Share links (and the hand-over of listen-together invites). `share(...)` from any menu opens [`ShareDialog`], which stores the list on the
//! share relays and copies the short link. Opening a link (clicked in a browser or chat, or a
//! second launch with `openwave://s/<id>`) arrives through [`LinkInbox`]: one song plays, a list
//! opens the import dialog on its preview.

mod dialog;

use std::time::Duration;

use dioxus::prelude::*;

use crate::backend::api;
use crate::backend::error::Coded;
use crate::backend::listen;
use crate::backend::share::{parse_open, SCHEME};
use crate::features::shell::slots::{OPEN_IMPORT, PENDING_IMPORT};
use crate::i18n::t;
use crate::model::Track;
use crate::player::controller;
use crate::store::datadir::OPEN_LINK;
use crate::ui::{toast, toast_error};

pub use dialog::ShareDialog;
pub(crate) use dialog::{copy, LinkBox};

#[derive(Clone, Debug, PartialEq)]
pub struct ShareRequest {
    pub name: String,
    pub tracks: Vec<Track>,
    /// One song (it plays on open) rather than a list.
    pub single: bool,
}

static REQUEST: GlobalSignal<Option<ShareRequest>> = Signal::global(|| None);

/// Open the share dialog for a list.
pub fn share_list(name: &str, tracks: &[Track]) {
    *REQUEST.write() = Some(ShareRequest {
        name: name.to_owned(),
        tracks: tracks.to_vec(),
        single: false,
    });
}

/// Open the share dialog for one song.
pub fn share_track(track: &Track) {
    *REQUEST.write() = Some(ShareRequest {
        name: track.title.to_string(),
        tracks: vec![track.clone()],
        single: true,
    });
}

/// Hand a link from the command line to the running window (this process or the first one).
pub fn hand_over(dir: &std::path::Path, args: impl Iterator<Item = String>) {
    let link = args.into_iter().find_map(|a| {
        if let Some(code) = listen::parse_join(&a) {
            return Some(format!("{}{code}", listen::SCHEME));
        }
        let (id, start) = parse_open(&a)?;
        let song = start.map(|n| format!("/{n}")).unwrap_or_default();
        Some(format!("{SCHEME}{id}{song}"))
    });
    if let Some(link) = link {
        let _ = std::fs::write(dir.join(OPEN_LINK), link);
    }
}

/// `start`: a song clicked on the share page; the list plays from it.
fn open(id: String, start: Option<usize>) {
    toast(t("share.opening"));
    spawn(async move {
        match api::open_share(&id).await {
            Ok(shared) if shared.single || start.is_some() => {
                let at = start.unwrap_or(0).min(shared.tracks.len() - 1);
                controller::play_now(&shared.tracks, at);
            }
            Ok(shared) => match api::shared_import(shared) {
                Ok(prepared) => {
                    *PENDING_IMPORT.write() = Some(prepared);
                    *OPEN_IMPORT.write() = true;
                }
                Err(e) => toast_error(e.user_message()),
            },
            Err(e) => toast_error(e.user_message()),
        }
    });
}

/// Watches for links handed over by [`hand_over`] (the file is taken, then removed).
#[component]
pub fn LinkInbox() -> Element {
    use_future(|| async {
        let path = api::data_dir().join(OPEN_LINK);
        loop {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let _ = std::fs::remove_file(&path);
                // An invite (`openwave://j/...`) joins a session; anything else is a share.
                if let (false, Some((id, start))) = (
                    crate::features::listen::open_invite(&text),
                    parse_open(&text),
                ) {
                    open(id, start);
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    rsx! {}
}
