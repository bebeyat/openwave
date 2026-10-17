//! The playlist title with inline rename: click (or the pencil, or the ⋯ menu) to edit,
//! Enter saves, Esc cancels, leaving the field saves a valid name.

use dioxus::prelude::*;

use super::logic::{validate_name, NameError};
use crate::features::tracks::state::update_library;
use crate::i18n::t;
use crate::store::{self, library::now_ms, library::PLAYLIST_NAME_MAX};
use crate::ui::{cx, next_id, Icon};

const FIELD: &str =
    "h-12 w-full rounded-md border bg-surface px-3 text-2xl font-bold tracking-tight text-fg \
focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent-text";

#[component]
fn NameInput(id: String, initial: String, editing: Signal<bool>) -> Element {
    let input_id = use_hook(|| next_id("rename"));
    let mut draft = use_signal(|| initial.clone());
    let mut error = use_signal(|| None::<NameError>);
    let mut finished = use_signal(|| false);
    let current = initial;

    // `report` shows a validation problem; blur passes false and quietly drops an invalid name.
    let finish = Callback::new(move |report: bool| {
        if *finished.peek() {
            return;
        }
        let raw = draft.peek().clone();
        let lib = store::library().get();
        let others: Vec<&str> = lib
            .playlists
            .iter()
            .filter(|p| p.id != id)
            .map(|p| p.name.as_str())
            .collect();
        match validate_name(&raw, &others) {
            Ok(clean) => {
                if clean != current {
                    update_library(|l| l.rename_playlist(&id, &clean, now_ms()));
                }
                finished.set(true);
                editing.set(false);
            }
            Err(e) if report => error.set(Some(e)),
            Err(_) => {
                finished.set(true);
                editing.set(false);
            }
        }
    });
    let focus_id = input_id.clone();
    let border = if error().is_some() {
        "border-danger"
    } else {
        "border-line-strong"
    };
    rsx! {
        div { class: "flex flex-col items-start gap-1.5",
            input {
                id: "{input_id}",
                r#type: "text",
                class: cx(&[FIELD, border]),
                value: "{draft}",
                maxlength: "{PLAYLIST_NAME_MAX}",
                "aria-label": t("playlist.name_label"),
                "aria-invalid": error().is_some().then_some("true"),
                onmounted: move |_| {
                    document::eval(&format!(
                        "var e=document.getElementById('{focus_id}');if(e){{e.focus();e.select();}}"
                    ));
                },
                oninput: move |e| {
                    draft.set(e.value());
                    error.set(None);
                },
                onkeydown: move |e| match e.key() {
                    Key::Enter => finish.call(true),
                    Key::Escape => {
                        e.stop_propagation();
                        finished.set(true);
                        editing.set(false);
                    }
                    _ => {}
                },
                onblur: move |_| finish.call(false),
            }
            if let Some(e) = error() {
                p { role: "alert", class: "flex items-center gap-1.5 text-sm text-danger-text",
                    Icon { name: "circle-alert", size: 16 }
                    {t(e.key())}
                }
            }
        }
    }
}

#[component]
pub fn TitleEditor(id: String, name: String, editing: Signal<bool>) -> Element {
    if editing() {
        return rsx! { NameInput { id, initial: name, editing } };
    }
    rsx! {
        h1 { class: "ow-hero-title text-fg",
            button {
                r#type: "button",
                title: t("playlist.rename_hint"),
                class: "group inline-flex max-w-full items-center gap-3 rounded-lg text-left hover:bg-hover",
                onclick: move |_| editing.set(true),
                span { class: "line-clamp-2 min-w-0 break-words", "{name}" }
                Icon { name: "pencil", size: 26, class: "flex-none text-muted opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" }
            }
        }
    }
}
