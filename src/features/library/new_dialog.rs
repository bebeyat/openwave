//! The one small "Yeni çalma listesi" dialog. Mounted once (inside the sidebar playlist slot) and
//! opened from anywhere with [`open_new_playlist`].

use dioxus::prelude::*;

use super::logic::{validate_name, NameError};
use crate::features::tracks::state::update_library;
use crate::i18n::{t, t_args};
use crate::routes::Route;
use crate::store::{self, library::new_id, library::now_ms};
use crate::ui::{toast_success, Button, ButtonVariant, Dialog, DialogSize, TextInput};

pub static OPEN_NEW_PLAYLIST: GlobalSignal<bool> = Signal::global(|| false);

pub fn open_new_playlist() {
    *OPEN_NEW_PLAYLIST.write() = true;
}

#[component]
pub fn NewPlaylistDialog() -> Element {
    let open = *OPEN_NEW_PLAYLIST.read();
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<NameError>);
    let nav = navigator();
    let close = Callback::new(move |()| {
        *OPEN_NEW_PLAYLIST.write() = false;
        name.set(String::new());
        error.set(None);
    });
    let create = Callback::new(move |()| {
        let raw = name.peek().clone();
        let lib = store::library().get();
        let others: Vec<&str> = lib.playlists.iter().map(|p| p.name.as_str()).collect();
        match validate_name(&raw, &others) {
            Ok(clean) => {
                let id = new_id();
                update_library(|l| l.create_playlist(&id, &clean, now_ms()));
                toast_success(t_args("toast.playlist_created", &[("name", &clean)]));
                close.call(());
                nav.push(Route::Playlist { id: id.to_string() });
            }
            Err(e) => error.set(Some(e)),
        }
    });
    rsx! {
        Dialog {
            open,
            on_close: move |()| close.call(()),
            title: t("playlist.new.title"),
            size: DialogSize::Sm,
            footer: Some(rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| close.call(()), {t("common.cancel")} }
                Button { variant: ButtonVariant::Primary, icon: "plus", onclick: move |_| create.call(()), {t("playlist.new.create")} }
            }),
            TextInput {
                label: t("playlist.name_label"),
                value: name(),
                placeholder: t("playlist.default_name"),
                error: error().map(|e| t(e.key())),
                oninput: move |v: String| {
                    name.set(v);
                    error.set(None);
                },
                onsubmit: move |()| create.call(()),
            }
        }
    }
}
