//! "Listeye kaydet": turns the current queue into a playlist.

use dioxus::prelude::*;

use crate::features::tracks::state::update_library;
use crate::i18n::{t, t_args};
use crate::player::controller as ctl;
use crate::store::library;
use crate::ui::{toast_success, Button, ButtonVariant, Dialog, DialogSize, TextInput};

fn save(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let tracks = ctl::QUEUE.peek().tracks();
    if tracks.is_empty() {
        return false;
    }
    let (id, now) = (library::new_id(), library::now_ms());
    update_library(|lib| {
        lib.create_playlist(&id, name, now)
            .add_to_playlist(&id, &tracks, now)
    });
    toast_success(t_args("queue.saved", &[("name", name)]));
    true
}

#[component]
pub fn SaveDialog(open: bool, on_close: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    use_effect(use_reactive!(|open| {
        if open {
            name.set(t("playlist.default_name"));
        }
    }));
    let empty = name().trim().is_empty();
    let submit = move || {
        if save(&name.peek()) {
            on_close.call(());
        }
    };
    rsx! {
        Dialog {
            open,
            on_close,
            size: DialogSize::Sm,
            title: t("queue.save"),
            footer: rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| on_close.call(()), {t("common.cancel")} }
                Button { disabled: empty, onclick: move |_| submit(), {t("playlist.new.create")} }
            },
            TextInput {
                label: t("playlist.name_label"),
                value: name(),
                oninput: move |v| name.set(v),
                onsubmit: move |_| submit(),
            }
        }
    }
}
