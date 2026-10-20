//! The small toolbar over synced lyrics: the per-track shift and the translation toggle.

use std::sync::Arc;

use dioxus::prelude::*;

use super::lyric_logic::{offset_label, OFFSET_STEP_MS};
use super::lyrics_state::{request_translation, TRANSLATE};
use crate::features::tracks::state::{library as library_state, update_library};
use crate::i18n::t;
use crate::model::{Id, Lyrics, Track};
use crate::ui::{IconButton, IconButtonSize};

fn shift_offset(id: &Id, delta: i32) {
    update_library(|lib| lib.set_lyric_offset(id, lib.lyric_offset(id) + delta));
}

#[component]
pub fn LyricsTools(track: Track, lyrics: Arc<Lyrics>, on_dark: bool) -> Element {
    let offset_ms = library_state().lyric_offset(&track.id);
    let translate = *TRANSLATE.read();
    let wanted = track.clone();
    let lines = lyrics.clone();
    use_effect(use_reactive!(|translate, wanted| {
        if translate {
            request_translation(
                &wanted,
                lines.synced.iter().map(|l| l.text.clone()).collect(),
            );
        }
    }));
    let (earlier_id, later_id) = (track.id.clone(), track.id.clone());
    let note = if on_dark { "text-fg/70" } else { "text-muted" };
    rsx! {
        div { class: "absolute right-4 top-2 z-10 flex items-center gap-1",
            IconButton {
                icon: "languages",
                label: t("lyrics.translate.toggle"),
                size: IconButtonSize::Sm,
                pressed: Some(translate),
                indicator: translate,
                onclick: move |_| {
                    let on = !*TRANSLATE.peek();
                    *TRANSLATE.write() = on;
                },
            }
            IconButton {
                icon: "minus",
                label: t("lyrics.offset.earlier"),
                size: IconButtonSize::Sm,
                onclick: move |_| shift_offset(&earlier_id, -OFFSET_STEP_MS),
            }
            span { class: "w-12 text-center text-xs tabular-nums {note}", {offset_label(offset_ms)} }
            IconButton {
                icon: "plus",
                label: t("lyrics.offset.later"),
                size: IconButtonSize::Sm,
                onclick: move |_| shift_offset(&later_id, OFFSET_STEP_MS),
            }
        }
    }
}
