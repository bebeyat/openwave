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
use crate::ui::{Button, ButtonVariant, Cover, Icon, IconButton, IconButtonSize};

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Downloading,
    Waiting,
    Failed,
}

/// Share of the run finished, for the bars (0 to 100).
fn run_percent(done: usize, left: usize) -> usize {
    (done * 100).checked_div(done + left).unwrap_or(0)
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
    let done = *RUN_DONE.read();
    let percent = run_percent(done, busy);
    let (waiting_n, failed_n) = (pending.len(), failed.len());
    rsx! {
        section { class: "mb-8 overflow-hidden rounded-lg border border-line bg-surface", "aria-label": t("downloads.queue.title"),
            div { class: "flex flex-wrap items-center gap-3 px-4 pb-3 pt-4",
                div { class: "min-w-0 flex-1",
                    p { class: "font-semibold text-fg", role: "status",
                        if busy > 0 { {t_count("downloads.pending", busy)} } else { {t("downloads.queue.title")} }
                    }
                    if busy > 0 {
                        p { class: "text-xs tabular-nums text-muted", "{done}/{done + busy} · {percent}%" }
                    }
                }
                if failed_n > 0 {
                    Button { variant: ButtonVariant::Ghost, icon: "refresh-cw", onclick: move |_| retry_failed(), {t("downloads.queue.retry_all")} }
                }
                if busy > 0 {
                    Button { variant: ButtonVariant::Ghost, icon: "x", onclick: move |_| cancel_all(), {t("downloads.cancel_all")} }
                }
            }
            if busy > 0 {
                div { class: "mx-4 mb-3 h-1 overflow-hidden rounded-full bg-line", "aria-hidden": "true",
                    div { class: "ow-accent-fill h-full rounded-full transition-[width] duration-(--ow-dur-panel)", style: "width: {percent}%" }
                }
            }
            div { class: "max-h-80 overflow-y-auto border-t border-line pb-2",
                if let Some(track) = active {
                    GroupHeading { label: t("downloads.queue.downloading") }
                    ul {
                        QueueRow { key: "a{track.id}", track, row: Row::Downloading, note: String::new() }
                    }
                }
                if waiting_n > 0 {
                    GroupHeading { label: t("downloads.queue.waiting"), count: waiting_n }
                    ul {
                        for track in pending {
                            QueueRow { key: "p{track.id}", track, row: Row::Waiting, note: String::new() }
                        }
                    }
                }
                if failed_n > 0 {
                    GroupHeading { label: t("downloads.queue.failed"), count: failed_n }
                    ul {
                        for (track, reason) in failed {
                            QueueRow { key: "f{track.id}", track, row: Row::Failed, note: reason }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GroupHeading(label: String, #[props(default)] count: usize) -> Element {
    rsx! {
        h3 { class: "px-4 pb-1 pt-3 text-xs font-semibold text-muted",
            "{label}"
            if count > 0 { span { class: "ml-1.5 tabular-nums", "· {count}" } }
        }
    }
}

#[component]
fn QueueRow(track: Track, row: Row, note: String) -> Element {
    let id = track.id.to_string();
    let artists = track.artists_label();
    let cover = track.cover.as_ref().map(|c| c.url.to_string());
    let (remove_id, retry_track) = (id.clone(), track.clone());
    let tone = if row == Row::Waiting {
        "opacity-80"
    } else {
        ""
    };
    rsx! {
        li { class: "group flex items-center gap-3 px-4 py-1.5 text-sm hover:bg-hover {tone}",
            div { class: "relative flex-none",
                Cover { src: cover, alt: "", size: 36 }
                if row == Row::Downloading {
                    span { class: "absolute inset-0 grid place-items-center rounded-[inherit] bg-black/45", "aria-hidden": "true",
                        span { class: "ow-spin inline-block h-4 w-4 rounded-full border-2 border-white/30 border-t-white" }
                    }
                }
            }
            div { class: "min-w-0 flex-1",
                div { class: "truncate font-medium text-fg", "{track.title}" }
                div { class: "truncate text-xs text-muted", "{artists}" }
            }
            if !note.is_empty() {
                span { class: "max-w-[40%] flex-none truncate text-xs text-danger-text", title: "{note}", "{note}" }
            }
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
    let percent = run_percent(done, left);
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

#[cfg(test)]
mod tests {
    use super::run_percent;

    #[test]
    fn run_percent_counts_finished_share() {
        assert_eq!(run_percent(0, 0), 0);
        assert_eq!(run_percent(20, 64), 23);
        assert_eq!(run_percent(5, 0), 100);
    }
}
