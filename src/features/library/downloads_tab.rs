//! "İndirilenler": tracks saved for offline listening, with their total size.

use dioxus::prelude::*;

use super::actions::{play_all, shuffle_all};
use crate::features::downloads::{self, ACTIVE, DOWNLOADS, PENDING};
use crate::features::tracks::common::IllustratedEmpty;
use crate::features::tracks::TrackList;
use crate::i18n::{t, t_args, t_count};
use crate::model::Track;
use crate::ui::{Button, ButtonVariant, GradientPlayButton, PlaySize};

/// "1,2 GB" / "340 MB".
pub fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{} MB", mb.round() as u64)
    }
}

#[component]
pub fn DownloadsTab() -> Element {
    let tracks: Vec<Track> = DOWNLOADS.read().iter().map(|d| d.track.clone()).collect();
    let pending = PENDING.read().len() + usize::from(ACTIVE.read().is_some());
    let total = size_text(downloads::total_bytes());
    let (a, b, list) = (tracks.clone(), tracks.clone(), tracks.clone());
    rsx! {
        if pending > 0 {
            div { class: "mb-4 flex items-center gap-3 rounded-lg border border-line px-4 py-3 text-sm", role: "status",
                span { class: "ow-spin inline-block h-4 w-4 rounded-full border-2 border-line border-t-accent", "aria-hidden": "true" }
                span { class: "flex-1 text-fg", {t_count("downloads.pending", pending)} }
                Button { variant: ButtonVariant::Ghost, onclick: move |_| downloads::cancel_pending(), {t("downloads.cancel")} }
            }
        }
        if !tracks.is_empty() {
            div { class: "mb-5 flex flex-wrap items-center gap-3",
                GradientPlayButton { size: PlaySize::Md, label: t("common.play"), onclick: move |_| play_all(&a) }
                Button { variant: ButtonVariant::Secondary, icon: "shuffle", onclick: move |_| shuffle_all(&b), {t("common.shuffle")} }
                span { class: "ml-2 text-sm font-medium text-muted",
                    {t_args("downloads.summary", &[("songs", &t_count("common.songs", tracks.len())), ("size", &total)])}
                }
            }
        }
        TrackList {
            sortable: true,
            tracks,
            label: t("library.tab.downloads"),
            on_remove: move |i: usize| {
                if let Some(track) = list.get(i) {
                    downloads::delete(&track.id);
                }
            },
            empty: rsx! {
                IllustratedEmpty {
                    icon: "download",
                    title: t("downloads.empty.title"),
                    text: t("downloads.empty.body"),
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_text(0), "0 MB");
        assert_eq!(size_text(5 * 1024 * 1024 + 400_000), "5 MB");
        assert_eq!(size_text(1536 * 1024 * 1024), "1.5 GB");
    }
}
