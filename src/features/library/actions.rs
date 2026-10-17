//! Things the library screens do: add files from disk, play or shuffle a list.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::features::tracks::actions::play_shuffled;
use crate::features::tracks::state::update_library;
use crate::i18n::t_count;
use crate::model::Track;
use crate::player::controller;
use crate::routes::Route;
use crate::ui::{toast_error, toast_success};

/// Pick audio files with the native dialog, store them as local files and show the count.
pub fn add_local_files() {
    let nav = navigator();
    spawn(async move {
        match api::import_audio().await {
            Ok(tracks) if tracks.is_empty() => {}
            Ok(tracks) => {
                update_library(|lib| lib.add_local_files(&tracks));
                toast_success(t_count("home.local_added", tracks.len()));
                nav.replace(Route::Library {
                    tab: "local".into(),
                });
            }
            Err(e) => toast_error(e.user_message()),
        }
    });
}

/// Pick a folder and add every audio file under it.
pub fn add_local_folder() {
    let nav = navigator();
    spawn(async move {
        match api::import_folder().await {
            Ok(tracks) if tracks.is_empty() => {}
            Ok(tracks) => {
                update_library(|lib| lib.add_local_files(&tracks));
                toast_success(t_count("home.local_added", tracks.len()));
                nav.replace(Route::Library {
                    tab: "local".into(),
                });
            }
            Err(e) => toast_error(e.user_message()),
        }
    });
}

/// Read the tags of every local track again and update the library.
pub fn refresh_local_tags(tracks: Vec<Track>) {
    spawn(async move {
        match api::refresh_local_tracks(tracks).await {
            Ok(fresh) => {
                update_library(|lib| lib.refresh_local_tracks(&fresh));
                toast_success(t_count("library.local.refreshed", fresh.len()));
            }
            Err(e) => toast_error(e.user_message()),
        }
    });
}

pub fn play_all(tracks: &[Track]) {
    if !tracks.is_empty() {
        controller::play_now(tracks, 0);
    }
}

pub fn shuffle_all(tracks: &[Track]) {
    play_shuffled(tracks);
}

pub fn open_import() {
    *crate::features::shell::slots::OPEN_IMPORT.write() = true;
}

pub fn go_to_search() {
    navigator().push(Route::Search { q: String::new() });
}

/// Run `f` once a dropdown menu has closed and handed focus back to its button; opening a dialog
/// or an input earlier would lose focus to that button.
pub fn after_menu_closes(f: impl FnOnce() + 'static) {
    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(220)).await;
        f();
    });
}
