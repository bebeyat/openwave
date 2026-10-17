//! "Çalma listeleri" tab: a grid of mosaic cards, "Beğenilenler" first when there are likes.

use dioxus::prelude::*;

use super::actions::{open_import, play_all};
use super::cover::{NewPlaylistCard, PlaylistCard};
use super::logic::mosaic;
use super::new_dialog::open_new_playlist;
use crate::features::tracks::common::IllustratedEmpty;
use crate::features::tracks::state::library;
use crate::i18n::{t, t_args, t_count};
use crate::routes::Route;
use crate::ui::{Button, ButtonVariant};

#[component]
pub fn PlaylistsTab() -> Element {
    let lib = library();
    let nav = navigator();
    if lib.playlists.is_empty() && lib.liked.is_empty() {
        return rsx! {
            IllustratedEmpty {
                icon: "list-music",
                title: t("library.empty.playlists.title"),
                text: t("library.empty.playlists.body"),
                Button { icon: "plus", onclick: move |_| open_new_playlist(), {t("library.empty.playlists.create")} }
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "download",
                    onclick: move |_| open_import(),
                    {t("library.empty.import")}
                }
            }
        };
    }
    let liked = lib.liked.clone();
    let liked_play = liked.clone();
    rsx! {
        div { class: "grid grid-cols-[repeat(auto-fill,minmax(var(--ow-card-w),1fr))] gap-3",
            if !liked.is_empty() {
                PlaylistCard {
                    title: t("library.tab.liked"),
                    subtitle: t_count("common.songs", liked.len()),
                    mosaic: mosaic(&liked),
                    icon: "heart",
                    liked: true,
                    play_label: t_args("library.play_named", &[("name", &t("library.tab.liked"))]),
                    on_open: move |()| {
                        nav.push(Route::Library { tab: "liked".into() });
                    },
                    on_play: move |()| play_all(&liked_play),
                }
            }
            for p in lib.playlists.iter() {
                {
                    let (id, tracks) = (p.id.to_string(), p.tracks.clone());
                    let art = super::logic::playlist_art(p);
                    let name = p.name.to_string();
                    rsx! {
                        PlaylistCard {
                            key: "{p.id}",
                            title: name.clone(),
                            subtitle: t_count("common.songs", tracks.len()),
                            mosaic: art,
                            can_play: !tracks.is_empty(),
                            play_label: t_args("library.play_named", &[("name", &name)]),
                            on_open: move |()| {
                                nav.push(Route::Playlist { id: id.clone() });
                            },
                            on_play: move |()| play_all(&tracks),
                        }
                    }
                }
            }
            NewPlaylistCard { on_click: move |()| open_new_playlist() }
        }
    }
}
