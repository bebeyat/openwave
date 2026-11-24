//! The download queue in "İndirilenler": the track downloading now, the ones waiting and the
//! ones that failed, each with its own cancel or retry (issue #40).

use dioxus::prelude::*;

use super::{
    cancel_active, cancel_all, dismiss_failed, remove_pending, retry_failed, ACTIVE, FAILED,
    PENDING, RUN_DONE,
};
use crate::i18n::{t, t_args, t_count};
use crate::model::Track;
use crate::routes::Route;
use crate::ui::{Button, ButtonVariant, Icon, IconButton, IconButtonSize};

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Downloading,
    Waiting,
    Failed,
}

#[component]
pub fn QueuePanel() -> Element {
    let active = ACTIVE.read().clone();
    let pending: Vec<Track> = PENDING.read().iter().cloned().collect();
    let failed = FAILED.read().clone();
    let busy = pending.len() + usize::from(active.is_some());
    if busy == 0 && failed.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "mb-5 rounded-lg border border-line", "aria-label": t("downloads.queue.title"),
            div { class: "flex flex-wrap items-center gap-3 border-b border-line px-4 py-3 text-sm",
                if busy > 0 {
                    span { class: "ow-spin inline-block h-4 w-4 rounded-full border-2 border-line border-t-accent", "aria-hidden": "true" }
                }
                span { class: "flex-1 font-semibold text-fg", role: "status",
                    if busy > 0 { {t_count("downloads.pending", busy)} } else { {t("downloads.queue.title")} }
                }
                if !failed.is_empty() {
                    Button { variant: ButtonVariant::Ghost, icon: "refresh-cw", onclick: move |_| retry_failed(), {t("downloads.queue.retry_all")} }
                }
                if busy > 0 {
                    Button { variant: ButtonVariant::Ghost, onclick: move |_| cancel_all(), {t("downloads.cancel_all")} }
                }
            }
            ul { class: "max-h-72 overflow-y-auto py-1",
                if let Some(track) = active {
                    QueueRow { key: "a{track.id}", track, row: Row::Downloading, note: t("downloads.queue.downloading") }
                }
                for track in pending {
                    QueueRow { key: "p{track.id}", track, row: Row::Waiting, note: t("downloads.queue.waiting") }
                }
                for (track, reason) in failed {
                    QueueRow { key: "f{track.id}", track, row: Row::Failed, note: reason }
                }
            }
        }
    }
}

#[component]
fn QueueRow(track: Track, row: Row, note: String) -> Element {
    let id = track.id.to_string();
    let artists = track.artists_label();
    let note_class = if row == Row::Failed {
        "text-danger-text"
    } else {
        "text-muted"
    };
    let (remove_id, retry_track) = (id.clone(), track.clone());
    rsx! {
        li { class: "flex items-center gap-3 px-4 py-1.5 text-sm",
            div { class: "min-w-0 flex-1",
                div { class: "truncate font-medium text-fg", "{track.title}" }
                div { class: "truncate text-xs text-muted", "{artists}" }
            }
            span { class: "max-w-[40%] flex-none truncate text-xs {note_class}", title: "{note}", "{note}" }
            if row == Row::Failed {
                IconButton {
                    icon: "refresh-cw",
                    label: t("downloads.queue.retry"),
                    size: IconButtonSize::Sm,
                    onclick: move |_| super::enqueue(std::slice::from_ref(&retry_track)),
                }
            }
            IconButton {
                icon: "x",
                label: t("downloads.queue.remove"),
                size: IconButtonSize::Sm,
                onclick: move |_| match row {
                    Row::Downloading => cancel_active(),
                    Row::Waiting => remove_pending(&remove_id),
                    Row::Failed => dismiss_failed(&remove_id),
                },
            }
        }
    }
}

/// Sidebar entry above Settings while songs download: "done/total" with a bar; opens the queue.
#[component]
pub fn SidebarProgress() -> Element {
    let left = PENDING.read().len() + usize::from(ACTIVE.read().is_some());
    if left == 0 {
        return rsx! {};
    }
    let done = *RUN_DONE.read();
    let total = done + left;
    let percent = done * 100 / total;
    let label = t_args(
        "downloads.ring",
        &[("done", &done.to_string()), ("total", &total.to_string())],
    );
    rsx! {
        Link {
            to: Route::Library { tab: "downloads".into() },
            "aria-label": "{label}",
            title: "{label}",
            class: "flex flex-col gap-2 rounded-md px-4 py-2.5 text-muted transition-colors duration-(--ow-dur-hover) hover:bg-hover hover:text-fg max-[1099px]:items-center max-[1099px]:px-1",
            div { class: "flex items-center gap-3.5 max-[1099px]:flex-col max-[1099px]:gap-1",
                Icon { name: "download", size: 22, class: "text-accent-text" }
                span { class: "flex-1 truncate text-[15px] font-medium max-[1099px]:sr-only", {t("downloads.queue.downloading")} }
                span { class: "text-xs font-semibold tabular-nums text-fg max-[1099px]:text-[11px]", "{done}/{total}" }
            }
            div { class: "h-1 w-full overflow-hidden rounded-full bg-line", "aria-hidden": "true",
                div { class: "ow-accent-fill h-full rounded-full transition-[width] duration-(--ow-dur-panel)", style: "width: {percent}%" }
            }
        }
    }
}
