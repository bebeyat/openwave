//! "Çalma listesine ekle": pick an existing playlist or create a new one inline.

use dioxus::prelude::*;

use super::state::{library, update_library};
use crate::i18n::{t, t_args};
use crate::model::Track;
use crate::store::library::{new_id, now_ms};
use crate::ui::{
    toast_success, Button, ButtonSize, ButtonVariant, Dialog, DialogSize, Icon, TextInput,
};

/// Add `tracks` to the playlist `id` and tell the user where they went.
pub fn add_tracks_to(id: &str, name: &str, tracks: &[Track]) {
    update_library(|lib| lib.add_to_playlist(id, tracks, now_ms()));
    toast_success(t_args("playlist.added", &[("name", name)]));
}

/// Create a playlist with `name` holding `tracks`.
pub fn create_playlist_with(name: &str, tracks: &[Track]) {
    let id = new_id();
    update_library(|lib| {
        lib.create_playlist(&id, name, now_ms())
            .add_to_playlist(&id, tracks, now_ms())
    });
    toast_success(t_args("playlist.added", &[("name", name.trim())]));
}

/// Open when `tracks` is `Some`; closes through `on_close` after an add or Esc.
#[component]
pub fn AddToPlaylistDialog(tracks: Option<Vec<Track>>, on_close: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let lib = library();
    let open = tracks.is_some();
    let items = tracks.clone().unwrap_or_default();
    let create_items = items.clone();
    let create = Callback::new(move |()| {
        let n = name();
        if !n.trim().is_empty() {
            create_playlist_with(&n, &create_items);
            name.set(String::new());
            on_close.call(());
        }
    });
    let can_create = !name().trim().is_empty();
    rsx! {
        Dialog {
            open,
            on_close,
            title: t("playlist.pick.title"),
            size: DialogSize::Sm,
            footer: Some(rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| on_close.call(()), {t("common.cancel")} }
            }),
            div { class: "flex flex-col gap-4",
                if !lib.playlists.is_empty() {
                    ul { class: "flex max-h-[240px] flex-col gap-1 overflow-y-auto", "aria-label": t("nav.playlists"),
                        for p in lib.playlists.iter() {
                            {
                                let (id, pname, list) = (p.id.to_string(), p.name.to_string(), items.clone());
                                rsx! {
                                    li { key: "{p.id}",
                                        button {
                                            r#type: "button",
                                            class: "flex h-10 w-full items-center gap-3 rounded-md px-3 text-left text-sm text-fg hover:bg-hover",
                                            onclick: move |_| {
                                                add_tracks_to(&id, &pname, &list);
                                                on_close.call(());
                                            },
                                            Icon { name: "list-music", size: 18, class: "text-muted" }
                                            span { class: "min-w-0 flex-1 truncate", "{p.name}" }
                                            span { class: "text-muted", {crate::i18n::t_count("common.songs", p.tracks.len())} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "flex flex-col gap-2",
                    TextInput {
                        label: t("playlist.pick.new"),
                        value: name(),
                        placeholder: t("playlist.name_label"),
                        oninput: move |v: String| name.set(v),
                        onsubmit: move |_| create.call(()),
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Sm,
                        icon: "plus",
                        disabled: !can_create,
                        onclick: move |_| create.call(()),
                        {t("playlist.new.create")}
                    }
                }
            }
        }
    }
}
