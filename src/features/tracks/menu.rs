//! The items of the track "⋯" menu (also used for right-click on a row).

use dioxus::prelude::*;

use super::actions::{open_album, open_artist};
use super::state::toggle_like;
use crate::features::downloads::{self, State as DlState};
use crate::i18n::t;
use crate::model::Track;
use crate::player::controller;
use crate::ui::{toast, MenuItem, MenuSeparator};

/// What the menu may offer besides the always-present items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuExtras {
    /// "Yukarı taşı" is offered.
    pub can_move_up: bool,
    /// "Aşağı taşı" is offered.
    pub can_move_down: bool,
    /// "Listeden kaldır" is offered.
    pub removable: bool,
}

#[component]
pub fn TrackMenuItems(
    track: Track,
    liked: bool,
    extras: MenuExtras,
    on_add_to_playlist: EventHandler<Track>,
    on_move: EventHandler<isize>,
    on_remove: EventHandler<()>,
) -> Element {
    let artist_id = track
        .artists
        .iter()
        .map(|a| a.id.to_string())
        .find(|id| !id.is_empty());
    let album_id = track
        .album
        .as_ref()
        .map(|a| a.id.to_string())
        .filter(|id| !id.is_empty());
    let (t_next, t_last, t_add) = (track.clone(), track.clone(), track.clone());
    let t_like = track.clone();
    let t_dl = track.clone();
    let t_share = (!track.is_local()).then(|| track.clone());
    let dl_state = if downloads::downloadable(&track) {
        Some(downloads::state(&track.id))
    } else {
        None
    };
    let dl_id = track.id.to_string();
    let like_label = if liked {
        t("track.unlike")
    } else {
        t("track.like")
    };
    rsx! {
        MenuItem {
            icon: "list-plus",
            label: t("track.menu.next"),
            onselect: move |_| {
                controller::add_next(std::slice::from_ref(&t_next));
                toast(t("track.toast.next"));
            },
        }
        MenuItem {
            icon: "list-end",
            label: t("track.menu.last"),
            onselect: move |_| {
                controller::add_last(std::slice::from_ref(&t_last));
                toast(t("track.toast.last"));
            },
        }
        MenuItem {
            icon: "list-music",
            label: t("track.add_to_playlist"),
            onselect: move |_| on_add_to_playlist.call(t_add.clone()),
        }
        if let Some(id) = artist_id {
            MenuItem { icon: "user", label: t("track.go_to_artist"), onselect: move |_| open_artist(&id) }
        }
        if let Some(id) = album_id {
            MenuItem { icon: "disc-3", label: t("track.go_to_album"), onselect: move |_| open_album(&id) }
        }
        MenuItem {
            icon: "heart",
            label: like_label,
            onselect: move |_| toggle_like(&t_like),
        }
        if let Some(t_share) = t_share {
            MenuItem {
                icon: "share-2",
                label: t("share.action"),
                onselect: move |_| crate::features::share::share_track(&t_share),
            }
        }
        match dl_state {
            Some(DlState::None) => rsx! {
                MenuItem { icon: "download", label: t("downloads.menu.download"), onselect: move |_| downloads::enqueue(std::slice::from_ref(&t_dl)) }
            },
            Some(DlState::Queued | DlState::Downloading) => rsx! {
                MenuItem { icon: "download", label: t("downloads.menu.downloading"), disabled: true, onselect: move |_| {} }
            },
            Some(DlState::Done) => rsx! {
                MenuItem { icon: "trash-2", label: t("downloads.menu.delete"), onselect: move |_| downloads::delete(&dl_id) }
            },
            None => rsx! {},
        }
        if extras.can_move_up || extras.can_move_down {
            MenuSeparator {}
        }
        if extras.can_move_up {
            MenuItem { icon: "arrow-up", label: t("track.move_up"), onselect: move |_| on_move.call(-1) }
        }
        if extras.can_move_down {
            MenuItem { icon: "arrow-down", label: t("track.move_down"), onselect: move |_| on_move.call(1) }
        }
        if extras.removable {
            MenuSeparator {}
            MenuItem {
                icon: "trash-2",
                label: t("track.remove_from_playlist"),
                danger: true,
                onselect: move |_| on_remove.call(()),
            }
        }
    }
}
