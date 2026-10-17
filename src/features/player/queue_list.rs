//! The queue tab of the drawer: now playing, up next (drag or menu to reorder), clear and save.

use dioxus::prelude::*;

use super::save_dialog::SaveDialog;
use crate::i18n::{t, t_count};
use crate::player::controller as ctl;
use crate::player::queue::QueueEntry;
use crate::ui::{
    Button, ButtonSize, ButtonVariant, Cover, CoverShape, CoverThumb, DropdownMenu, EmptyState,
    GradientPlayButton, Icon, MenuAlign, MenuItem, PlaySize,
};
use crate::util::format_time;

/// Pointer drag for the list (see `assets/queue_drag.js`).
const QUEUE_DRAG_JS: &str = include_str!("../../../assets/queue_drag.js");

#[component]
pub fn QueuePanel(#[props(default)] hero: bool) -> Element {
    let mut saving = use_signal(|| false);
    use_future(|| async {
        let mut drag = document::eval(QUEUE_DRAG_JS);
        while let Ok(v) = drag.recv::<serde_json::Value>().await {
            let uid = v["uid"].as_u64();
            let before = v["before"].as_u64().unwrap_or(0);
            if let Some(uid) = uid {
                ctl::move_uid_before(uid, before);
            }
        }
    });
    use_drop(|| {
        document::eval("window.__owQueueDrag && window.__owQueueDrag();");
    });

    let queue = ctl::QUEUE.read().clone();
    let Some(current) = queue.current().cloned() else {
        return rsx! {
            EmptyState {
                icon: "list-music",
                title: t("queue.empty.title"),
                text: t("queue.empty.body"),
            }
        };
    };
    let upcoming: Vec<QueueEntry> = queue.upcoming().to_vec();
    let last = upcoming.len().saturating_sub(1);
    let status = *ctl::STATUS.read();
    rsx! {
        div { class: "flex h-full flex-col",
            div { class: "min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-3 py-3", "data-qscroll": "true",
                h3 { class: "ow-display px-2 pb-2 text-base font-bold text-fg", {t("queue.now_playing")} }
                if hero {
                    HeroNow { entry: current, playing: status.is_active() }
                } else {
                    NowRow { entry: current, playing: status.is_active() }
                }
                h3 { class: "ow-display mt-5 flex items-baseline justify-between px-2 pb-2 text-base font-bold text-fg",
                    span { {t("queue.up_next")} }
                    span { class: "text-xs font-normal text-muted", {t_count("common.songs", upcoming.len())} }
                }
                if upcoming.is_empty() {
                    p { class: "px-2 py-3 text-sm text-muted", {t("queue.up_next.empty")} }
                } else {
                    ul { "data-qlist": "true", "aria-label": t("queue.up_next"),
                        for (i , entry) in upcoming.into_iter().enumerate() {
                            QueueRow { key: "{entry.uid}", entry, first: i == 0, last: i == last }
                        }
                    }
                }
            }
            div { class: "flex flex-none items-center justify-between gap-2 border-t border-line px-3 py-2",
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    icon: "trash-2",
                    onclick: move |_| ctl::clear_queue(),
                    {t("queue.clear")}
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    size: ButtonSize::Sm,
                    icon: "list-plus",
                    onclick: move |_| saving.set(true),
                    {t("queue.save_short")}
                }
            }
            SaveDialog { open: saving(), on_close: move |_| saving.set(false) }
        }
    }
}

fn cover_src(entry: &QueueEntry) -> Option<String> {
    entry.track.cover.as_ref().map(|c| c.url.to_string())
}

/// The docked panel's "now playing" card: big cover, title, artist and the round play button.
#[component]
fn HeroNow(entry: QueueEntry, playing: bool) -> Element {
    let artists = entry.track.artists_label();
    let label = if playing {
        t("player.pause")
    } else {
        t("player.play")
    };
    rsx! {
        div { class: "flex flex-col gap-3 rounded-lg border border-line bg-surface p-3",
            div { class: "mx-auto w-full max-w-[clamp(120px,calc(100vh-640px),224px)]",
                Cover {
                    src: cover_src(&entry),
                    alt: "",
                    shape: CoverShape::Card,
                    class: "ow-cover-glow",
                }
            }
            div { class: "flex items-center gap-3 px-1",
                div { class: "min-w-0 flex-1",
                    p { class: "ow-display truncate text-lg font-bold text-fg", title: "{entry.track.title}", "{entry.track.title}" }
                    p { class: "truncate text-sm text-muted", "{artists}" }
                }
                GradientPlayButton {
                    playing,
                    label,
                    size: PlaySize::Md,
                    class: "h-11! w-11!",
                    onclick: move |_| ctl::toggle_pause(),
                }
            }
        }
    }
}

#[component]
fn NowRow(entry: QueueEntry, playing: bool) -> Element {
    let artists = entry.track.artists_label();
    let state = if playing {
        t("player.pause")
    } else {
        t("player.play")
    };
    rsx! {
        div { class: "ow-trow flex h-16 items-center gap-3 rounded-md px-2",
            "aria-current": "true",
            CoverThumb { src: cover_src(&entry), size: 44u32 }
            div { class: "min-w-0 flex-1",
                p { class: "truncate text-sm font-semibold text-accent-text", "{entry.track.title}" }
                p { class: "truncate text-xs text-muted", "{artists}" }
            }
            span { class: "text-xs tabular-nums text-muted", {format_time(entry.track.duration_ms as f64 / 1000.0)} }
            button {
                r#type: "button",
                class: "flex h-9 w-9 flex-none items-center justify-center rounded-full text-fg hover:bg-press",
                "aria-label": "{state}",
                onclick: move |_| ctl::toggle_pause(),
                Icon { name: if playing { "pause" } else { "play" }, size: 18, filled: true }
            }
        }
    }
}

#[component]
fn QueueRow(entry: QueueEntry, first: bool, last: bool) -> Element {
    let uid = entry.uid;
    let artists = entry.track.artists_label();
    let duration = format_time(entry.track.duration_ms as f64 / 1000.0);
    let title = entry.track.title.to_string();
    let play_label = format!("{} {}", t("common.play"), title);
    rsx! {
        li {
            "data-uid": "{uid}",
            class: "ow-trow group flex h-16 list-none items-center gap-1 rounded-md",
            span {
                "data-qhandle": "true",
                title: t("queue.drag"),
                "aria-hidden": "true",
                class: "flex h-9 w-6 flex-none cursor-grab touch-none items-center justify-center text-muted hover:text-fg",
                Icon { name: "grip-vertical", size: 16 }
            }
            button {
                r#type: "button",
                class: "flex min-w-0 flex-1 items-center gap-3 rounded-sm py-1 text-left",
                "aria-label": "{play_label}",
                onclick: move |_| ctl::play_uid(uid),
                CoverThumb { src: cover_src(&entry), size: 44u32 }
                div { class: "min-w-0 flex-1",
                    p { class: "truncate text-sm font-semibold text-fg", title: "{title}", "{title}" }
                    p { class: "truncate text-xs text-muted", title: "{artists}", "{artists}" }
                }
                span { class: "text-xs tabular-nums text-muted", "{duration}" }
            }
            DropdownMenu { icon: "ellipsis", aria_label: t("common.more"), align: MenuAlign::End,
                MenuItem {
                    icon: "play",
                    label: t("queue.play_now"),
                    onselect: move |_| ctl::play_uid(uid),
                }
                MenuItem {
                    icon: "arrow-up",
                    label: t("queue.move_up"),
                    disabled: first,
                    onselect: move |_| ctl::move_uid_by(uid, -1),
                }
                MenuItem {
                    icon: "arrow-down",
                    label: t("queue.move_down"),
                    disabled: last,
                    onselect: move |_| ctl::move_uid_by(uid, 1),
                }
                MenuItem {
                    icon: "x",
                    label: t("queue.remove"),
                    danger: true,
                    onselect: move |_| ctl::remove_uid(uid),
                }
            }
        }
    }
}
