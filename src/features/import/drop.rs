//! Drag and drop of audio files onto the window: they join "Bilgisayarım" without opening the
//! import dialog. Non-audio files are ignored with a plain message.

use std::path::PathBuf;

use dioxus::dioxus_core::spawn_forever;
use dioxus::html::HasFileData;
use dioxus::prelude::*;

use super::save::add_paths;
use super::state::is_audio_path;
use crate::i18n::{t, t_count};
use crate::ui::{toast, toast_error, toast_success};

/// Keep the browser from navigating to a dropped file.
pub fn handle_drag_over(e: DragEvent) {
    e.prevent_default();
}

pub fn handle_drop(e: DragEvent) {
    e.prevent_default();
    let paths: Vec<PathBuf> = e.files().into_iter().map(|f| f.path()).collect();
    add_dropped(paths);
}

fn add_dropped(paths: Vec<PathBuf>) {
    let audio: Vec<PathBuf> = paths.into_iter().filter(|p| is_audio_path(p)).collect();
    if audio.is_empty() {
        toast(t("import.drop.none"));
        return;
    }
    spawn_forever(async move {
        match add_paths(audio).await {
            Ok(0) => toast(t("import.drop.none")),
            Ok(n) => toast_success(t_count("import.drop.added", n)),
            Err(message) => toast_error(message),
        }
    });
}
