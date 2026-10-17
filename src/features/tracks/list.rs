//! `TrackList`: the shared song list (artist, album, search, and later library/playlist pages).
//!
//! ```ignore
//! TrackList { tracks, on_reorder: move |(from, to)| .., on_remove: move |index| .. }
//! ```
//! * `tracks`: rows in display order. Playing a row calls `controller::play_now(&tracks, index)`.
//! * `loading` shows skeleton rows (only while `tracks` is empty), `error` + `on_retry` an error
//!   state, `empty` replaces the default empty state.
//! * `on_reorder((from, to))`: enables drag-reorder, Alt+Up/Down on a row and the menu items
//!   "Yukarı/Aşağı taşı"; `to` is the final index (same meaning as `Library::move_in_playlist`).
//! * `on_remove(index)`: shows "Listeden kaldır" in the row menu.
//! * `framed` (default) puts the rows in a rounded glass table; `header` adds the sticky column
//!   header. `extra` + `extra_label` add one more text column (same length as `tracks`).
//! * Above 200 rows only the rows near the viewport are rendered (scroll container `#ow-main`).

use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use dioxus::prelude::*;

use super::common::IllustratedEmpty;
use super::playlist_dialog::AddToPlaylistDialog;
use super::row::{columns, TrackRow};
use super::state::library;
use super::window::{move_target, window, ROW_H, VIRTUAL_THRESHOLD};
use crate::i18n::{t, t_count};
use crate::model::Track;
use crate::player::controller::{self, NOW_PLAYING};
use crate::store::{self, DoubleClickAction};
use crate::ui::{next_id, toast, Button, ButtonSize, ButtonVariant, Icon, Skeleton};

const SKELETON_ROWS: usize = 8;

fn scroll_probe_js(id: &str) -> String {
    format!(
        r#"(function(){{var id="{id}",main=document.getElementById('ow-main');if(!main)return;var queued=false;
function send(){{queued=false;var e=document.getElementById(id);if(!e){{main.removeEventListener('scroll',on);window.removeEventListener('resize',on);return;}}
var m=main.getBoundingClientRect(),r=e.getBoundingClientRect();dioxus.send([m.top-r.top,m.height]);}}
function on(){{if(!queued){{queued=true;requestAnimationFrame(send);}}}}
main.addEventListener('scroll',on,{{passive:true}});window.addEventListener('resize',on);send();}})();"#
    )
}

/// Keeps `(offset, viewport)` of a long list up to date while it is virtualized.
fn use_scroll_metrics(list_id: String, active: bool) -> Signal<(f64, f64)> {
    let mut metrics = use_signal(|| (0.0, 800.0));
    use_effect(use_reactive!(|active| {
        if active {
            let js = scroll_probe_js(&list_id);
            spawn(async move {
                let mut probe = document::eval(&js);
                while let Ok(v) = probe.recv::<(f64, f64)>().await {
                    metrics.set(v);
                }
            });
        }
    }));
    metrics
}

#[component]
fn SkeletonRows() -> Element {
    rsx! {
        div { "aria-busy": "true", role: "status",
            span { class: "sr-only", {t("common.loading")} }
            for i in 0..SKELETON_ROWS {
                div { key: "{i}", class: "flex h-14 items-center gap-3 px-2",
                    Skeleton { class: "h-4 w-6" }
                    Skeleton { class: "h-10 w-10" }
                    div { class: "flex flex-1 flex-col gap-2",
                        Skeleton { class: "h-3.5 w-1/3" }
                        Skeleton { class: "h-3 w-1/5" }
                    }
                }
            }
        }
    }
}

/// The bar above a list: "Seç" to start, then the count and the bulk actions.
#[component]
fn SelectionBar(
    selecting: bool,
    count: usize,
    total: usize,
    removable: bool,
    on_toggle: EventHandler<()>,
    on_all: EventHandler<()>,
    on_queue: EventHandler<()>,
    on_playlist: EventHandler<()>,
    on_download: EventHandler<()>,
    on_remove: EventHandler<()>,
) -> Element {
    if !selecting {
        return rsx! {
            div { class: "mb-2 flex justify-end",
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, icon: "check", onclick: move |_| on_toggle.call(()), {t("bulk.select")} }
            }
        };
    }
    let none = count == 0;
    let all_label = if count == total {
        t("bulk.select_none")
    } else {
        t("bulk.select_all")
    };
    rsx! {
        div { class: "ow-panel sticky top-2 z-20 mb-3 flex flex-wrap items-center gap-2 rounded-lg px-3 py-2", role: "toolbar", "aria-label": t("bulk.toolbar"),
            span { class: "mr-1 text-sm font-semibold text-fg", role: "status", {t_count("bulk.count", count)} }
            Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, onclick: move |_| on_all.call(()), "{all_label}" }
            div { class: "ml-auto flex flex-wrap items-center gap-2",
                Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm, icon: "list-end", disabled: none, onclick: move |_| on_queue.call(()), {t("bulk.queue")} }
                Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm, icon: "list-plus", disabled: none, onclick: move |_| on_playlist.call(()), {t("bulk.playlist")} }
                Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm, icon: "download", disabled: none, onclick: move |_| on_download.call(()), {t("bulk.download")} }
                if removable {
                    Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm, icon: "trash-2", disabled: none, onclick: move |_| on_remove.call(()), {t("bulk.remove")} }
                }
                Button { variant: ButtonVariant::Primary, size: ButtonSize::Sm, onclick: move |_| on_toggle.call(()), {t("bulk.done")} }
            }
        }
    }
}

#[component]
pub fn TrackList(
    tracks: Vec<Track>,
    #[props(default)] loading: bool,
    #[props(default)] error: Option<String>,
    #[props(default)] on_retry: Option<EventHandler<()>>,
    #[props(default)] empty: Option<Element>,
    #[props(default)] on_reorder: Option<EventHandler<(usize, usize)>>,
    #[props(default)] on_remove: Option<EventHandler<usize>>,
    #[props(default = true)] show_album: bool,
    #[props(default = true)] framed: bool,
    #[props(default = true)] header: bool,
    #[props(default)] extra: Option<Vec<String>>,
    #[props(into, default)] extra_label: String,
    /// Accessible name of the list.
    #[props(into, default)]
    label: Option<String>,
    /// Playing a row plays that song alone, followed by similar songs, instead of the rows after
    /// it (search results: the next result is not what the user wants to hear next).
    #[props(default)]
    similar_after: bool,
) -> Element {
    let list_id = use_hook(|| next_id("tracklist"));
    let mut dialog = use_signal(|| None::<Vec<Track>>);
    let mut selecting = use_signal(|| false);
    let mut selected = use_signal(BTreeSet::<usize>::new);
    // A changed list invalidates row indices: start the selection over.
    let len_now = tracks.len();
    use_effect(use_reactive!(|len_now| {
        let _ = len_now;
        if !selected.peek().is_empty() {
            selected.write().clear();
        }
    }));
    let mut drag_from = use_signal(|| None::<usize>);
    let mut drag_over = use_signal(|| None::<usize>);
    let virtual_on = tracks.len() > VIRTUAL_THRESHOLD;
    let metrics = use_scroll_metrics(list_id.clone(), virtual_on);
    let lib = library();
    let liked: HashSet<&str> = lib.liked.iter().map(|t| t.id.as_str()).collect();
    let downloaded = crate::features::downloads::downloaded_ids();
    let now = NOW_PLAYING.read().clone();
    let shared = Rc::new(tracks.clone());

    if tracks.is_empty() {
        if loading {
            return rsx! { SkeletonRows {} };
        }
        if let Some(message) = error {
            return rsx! {
                IllustratedEmpty { icon: "circle-alert", title: message,
                    if let Some(retry) = on_retry {
                        Button { variant: ButtonVariant::Secondary, icon: "refresh-cw", onclick: move |_| retry.call(()), {t("common.retry")} }
                    }
                }
            };
        }
        return match empty {
            Some(e) => e,
            None => {
                rsx! { IllustratedEmpty { icon: "music", title: t("tracks.empty.title"), text: t("tracks.empty.body") } }
            }
        };
    }

    let play_set = shared.clone();
    let on_play = EventHandler::new(move |i: usize| {
        let Some(track) = play_set.get(i) else { return };
        if NOW_PLAYING.peek().track_id.as_ref() == Some(&track.id) {
            controller::toggle_pause();
        } else if similar_after {
            controller::play_with_similar(track);
        } else {
            controller::play_now(&play_set, i);
        }
    });
    let double_set = shared.clone();
    let on_double = EventHandler::new(move |i: usize| {
        let Some(track) = double_set.get(i) else {
            return;
        };
        match store::settings().get().double_click {
            DoubleClickAction::PlayNow if similar_after => controller::play_with_similar(track),
            DoubleClickAction::PlayNow => controller::play_now(&double_set, i),
            DoubleClickAction::AddToQueue => {
                controller::add_last(std::slice::from_ref(track));
                toast(t("track.toast.last"));
            }
        }
    });
    let len = tracks.len();
    let focus_id = list_id.clone();
    let on_move = EventHandler::new(move |(i, delta): (usize, isize)| {
        if let (Some(cb), Some(to)) = (on_reorder, move_target(i, delta, len)) {
            cb.call((i, to));
            // The row is re-created at its new place: give keyboard focus back to it.
            let js = format!(
                "document.querySelector('#{focus_id} [data-index=\"{to}\"] button')?.focus()"
            );
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                document::eval(&js);
            });
        }
    });
    // Pointer-based reorder (OS drag-and-drop is unreliable inside the desktop webview): press the
    // grip of a row, move over another row, release.
    let finish_drag = Callback::new(move |apply: bool| {
        let from = drag_from.peek().to_owned();
        let to = drag_over.peek().to_owned();
        if let (true, Some(cb), Some(from), Some(to)) = (apply, on_reorder, from, to) {
            if from != to {
                cb.call((from, to));
            }
        }
        drag_from.set(None);
        drag_over.set(None);
    });
    let on_remove_row = EventHandler::new(move |i: usize| {
        if let Some(cb) = on_remove {
            cb.call(i);
        }
    });

    let (offset, viewport) = metrics();
    let w = window(len, offset, viewport, ROW_H);
    let reorderable = on_reorder.is_some();
    let removable = on_remove.is_some();
    let frame = if framed {
        "ow-table overflow-clip rounded-lg border border-line bg-surface"
    } else {
        ""
    };
    let cols = columns(show_album, extra.is_some());
    let head_pad = if reorderable { "pl-8 pr-4" } else { "px-4" };
    let rows_class = if drag_from().is_some() {
        "cursor-grabbing select-none"
    } else {
        ""
    };
    let rows_pad = if framed { "p-2" } else { "" };
    let picked: Vec<Track> = selected
        .read()
        .iter()
        .filter_map(|&i| tracks.get(i).cloned())
        .collect();
    let (queue_set, playlist_set, download_set) = (picked.clone(), picked.clone(), picked.clone());
    rsx! {
        if framed && len > 1 {
            SelectionBar {
                selecting: selecting(),
                count: picked.len(),
                total: len,
                removable,
                on_toggle: move |_| {
                    let on = !selecting();
                    selecting.set(on);
                    if !on {
                        selected.write().clear();
                    }
                },
                on_all: move |_| {
                    let next = if selected.peek().len() == len { BTreeSet::new() } else { (0..len).collect() };
                    selected.set(next);
                },
                on_queue: move |_| {
                    controller::add_last(&queue_set);
                    toast(t("track.toast.last"));
                },
                on_playlist: move |_| dialog.set(Some(playlist_set.clone())),
                on_download: move |_| crate::features::downloads::enqueue(&download_set),
                on_remove: move |_| {
                    if let Some(cb) = on_remove {
                        // Highest index first, so earlier removals do not shift later ones.
                        let order: Vec<usize> = selected.peek().iter().rev().copied().collect();
                        for i in order {
                            cb.call(i);
                        }
                        selected.write().clear();
                        selecting.set(false);
                    }
                },
            }
        }
        div { class: "{frame}",
            if header && framed {
                div {
                    "aria-hidden": "true",
                    class: "sticky top-0 z-10 grid h-10 items-center gap-3 border-b border-line bg-bg/90 {head_pad} text-xs font-semibold text-muted {cols}",
                    span { class: "text-center", "#" }
                    span { {t("tracks.col.title")} }
                    if show_album {
                        span { {t("tracks.col.album")} }
                    }
                    if extra.is_some() {
                        span { "{extra_label}" }
                    }
                    span { class: "flex justify-end", Icon { name: "clock", size: 14 } }
                    span {}
                    span {}
                }
            }
            div {
                id: "{list_id}",
                role: "list",
                "aria-label": label,
                class: "{rows_pad} {rows_class}",
                onmouseup: move |_| finish_drag.call(true),
                onmouseleave: move |_| finish_drag.call(false),
                if w.pad_top > 0.0 {
                    div { "aria-hidden": "true", style: "height:{w.pad_top}px" }
                }
                for i in w.start..w.end {
                    {
                        let track = tracks[i].clone();
                        let is_current = now.track_id.as_ref() == Some(&track.id);
                        let extra_text = extra.as_ref().and_then(|x| x.get(i).cloned());
                        rsx! {
                            TrackRow {
                                key: "{track.id}",
                                liked: liked.contains(track.id.as_str()),
                                downloaded: downloaded.contains(track.id.as_str()),
                                current: is_current,
                                playing: is_current && now.playing,
                                track,
                                index: i,
                                count: len,
                                show_album,
                                extra: extra_text,
                                reorderable,
                                removable,
                                dragging: drag_from() == Some(i),
                                drop_target: drag_over() == Some(i) && drag_from().is_some_and(|f| f != i),
                                drop_below: drag_from().is_some_and(|f| f < i),
                                on_play,
                                on_double,
                                selecting: selecting(),
                                selected: selected.read().contains(&i),
                                on_select: move |i: usize| {
                                    let mut set = selected.write();
                                    if !set.remove(&i) {
                                        set.insert(i);
                                    }
                                },
                                on_add_to_playlist: move |t: Track| dialog.set(Some(vec![t])),
                                on_remove: on_remove_row,
                                on_move,
                                on_drag_start: move |i| drag_from.set(Some(i)),
                                on_drag_over: move |i| {
                                    if *drag_over.peek() != Some(i) {
                                        drag_over.set(Some(i));
                                    }
                                },
                            }
                        }
                    }
                }
                if w.pad_bottom > 0.0 {
                    div { "aria-hidden": "true", style: "height:{w.pad_bottom}px" }
                }
            }
        }
        AddToPlaylistDialog { tracks: dialog(), on_close: move |_| dialog.set(None) }
    }
}
