//! `/playlist/:id`: mosaic header, inline rename, Play / Shuffle, drag-reorderable track list.

use dioxus::prelude::*;

use super::actions::{after_menu_closes, go_to_search, play_all, shuffle_all};
use super::cover::CoverMosaic;
use super::logic::{backdrop, playlist_art, playlist_summary};
use super::title_editor::TitleEditor;
use crate::features::shell::pages::{BackButton, Page};
use crate::features::tracks::common::IllustratedEmpty;
use crate::features::tracks::hero::{DetailHero, HeroButton};
use crate::features::tracks::state::{library, update_library};
use crate::features::tracks::TrackList;
use crate::i18n::{t, t_args};
use crate::routes::Route;
use crate::store::library::now_ms;
use crate::ui::{
    toast, Button, ButtonVariant, Dialog, DialogSize, DropdownMenu, GradientPlayButton, MenuAlign,
    MenuItem, MenuSeparator, PlaySize,
};

#[component]
fn NotFound() -> Element {
    let nav = navigator();
    rsx! {
        Page { title: t("detail.playlist"), back: true,
            IllustratedEmpty {
                icon: "list-music",
                title: t("playlist.not_found.title"),
                text: t("playlist.not_found.body"),
                Button {
                    icon: "library",
                    onclick: move |_| {
                        nav.replace(Route::Library { tab: "playlists".into() });
                    },
                    {t("playlist.not_found.back")}
                }
            }
        }
    }
}

#[component]
fn DeleteDialog(id: String, name: String, open: bool, on_close: EventHandler<()>) -> Element {
    let nav = navigator();
    let shown = name.clone();
    rsx! {
        Dialog {
            open,
            on_close,
            title: t_args("playlist.delete.title", &[("name", &shown)]),
            description: Some(t("playlist.delete.body")),
            size: DialogSize::Sm,
            footer: Some(rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| on_close.call(()), {t("common.cancel")} }
                Button {
                    variant: ButtonVariant::Danger,
                    icon: "trash-2",
                    onclick: move |_| {
                        update_library(|l| l.delete_playlist(&id));
                        toast(t_args("playlist.deleted", &[("name", &name)]));
                        on_close.call(());
                        nav.replace(Route::Library { tab: "playlists".into() });
                    },
                    {t("playlist.delete.confirm")}
                }
            }),
            span {}
        }
    }
}

/// Pick a picture for the playlist and keep it as its cover.
fn change_cover(id: String) {
    spawn(async move {
        match crate::backend::api::pick_cover_image().await {
            Ok(Some(cover)) => {
                update_library(move |l| l.set_playlist_cover(&id, Some(cover.clone()), now_ms()))
            }
            Ok(None) => {}
            Err(e) => {
                use crate::backend::Coded;
                crate::ui::toast_error(e.user_message());
            }
        }
    });
}

#[component]
fn PlaylistView(playlist: crate::model::Playlist) -> Element {
    let editing = use_signal(|| false);
    let mut confirm = use_signal(|| false);
    let id = playlist.id.to_string();
    let name = playlist.name.to_string();
    let tracks = playlist.tracks.clone();
    let empty = tracks.is_empty();
    let (play_set, shuffle_set, remove_set) = (tracks.clone(), tracks.clone(), tracks.clone());
    let (download_set, share_set) = (tracks.clone(), tracks.clone());
    let share_name = name.clone();
    let shareable = tracks.iter().any(|t| !t.is_local());
    let (reorder_id, remove_id) = (id.clone(), id.clone());
    let (cover_id, uncover_id) = (id.clone(), id.clone());
    let has_cover = playlist.cover.as_deref().is_some_and(|c| !c.is_empty());
    let mut start_edit = editing;
    rsx! {
        div { class: "ow-enter ow-page pb-10 pt-4",
            BackButton {}
            DetailHero {
                backdrop: backdrop(&tracks),
                meta: format!("{} · {}", t("detail.playlist"), playlist_summary(&playlist)),
                art: rsx! { CoverMosaic { mosaic: playlist_art(&playlist), size: 176 } },
                title_slot: rsx! { TitleEditor { id: id.clone(), name: name.clone(), editing } },
                actions: rsx! {
                    GradientPlayButton {
                        size: PlaySize::Md,
                        label: t("common.play"),
                        disabled: empty,
                        onclick: move |_| play_all(&play_set),
                    }
                    HeroButton { icon: "shuffle", disabled: empty, onclick: move |_| shuffle_all(&shuffle_set), {t("common.shuffle")} }
                    crate::features::downloads::DownloadAllButton { tracks: download_set.clone() }
                    HeroButton {
                        icon: "share-2",
                        disabled: !shareable,
                        onclick: move |_| crate::features::share::share_list(&share_name, &share_set),
                        {t("share.action")}
                    }
                    div { class: "ow-hero-menu",
                    DropdownMenu { aria_label: t("common.more"), icon: "ellipsis", align: MenuAlign::Start,
                        MenuItem {
                            icon: "pencil",
                            label: t("common.rename"),
                            onselect: move |()| after_menu_closes(move || start_edit.set(true)),
                        }
                        MenuItem {
                            icon: "image",
                            label: t("playlist.cover.change"),
                            onselect: move |()| change_cover(cover_id.clone()),
                        }
                        if has_cover {
                            MenuItem {
                                icon: "x",
                                label: t("playlist.cover.remove"),
                                onselect: move |()| {
                                    let id = uncover_id.clone();
                                    update_library(move |l| l.set_playlist_cover(&id, None, now_ms()));
                                },
                            }
                        }
                        MenuSeparator {}
                        MenuItem {
                            icon: "trash-2",
                            label: t("playlist.delete.menu"),
                            danger: true,
                            onselect: move |()| after_menu_closes(move || confirm.set(true)),
                        }
                    }
                    }
                },
            }
            TrackList {
                sortable: true,
                tracks: tracks.clone(),
                label: name.clone(),
                on_reorder: move |(from, to): (usize, usize)| {
                    update_library(|l| l.move_in_playlist(&reorder_id, from, to, now_ms()));
                },
                on_remove: move |i: usize| {
                    if let Some(track) = remove_set.get(i) {
                        update_library(|l| l.remove_from_playlist(&remove_id, &track.id, now_ms()));
                    }
                },
                empty: rsx! {
                    IllustratedEmpty {
                        icon: "list-music",
                        title: t("playlist.empty.title"),
                        text: t("playlist.empty.body"),
                        Button { icon: "search", onclick: move |_| go_to_search(), {t("home.action.search")} }
                    }
                },
            }
            DeleteDialog { id, name, open: confirm(), on_close: move |()| confirm.set(false) }
        }
    }
}

#[component]
pub fn Playlist(id: String) -> Element {
    let lib = library();
    match lib.playlist(&id) {
        Some(p) => rsx! { PlaylistView { key: "{id}", playlist: p.clone() } },
        None => rsx! { NotFound {} },
    }
}
