//! Listening stats: totals, most played songs and artists, all from the local play history.

pub mod logic;

use dioxus::prelude::*;

use crate::features::shell::pages::Page;
use crate::features::tracks::list::TrackList;
use crate::features::tracks::state::library;
use crate::i18n::t;
use crate::routes::Route;
use crate::ui::EmptyState;

const TOP: usize = 10;

#[component]
fn Tile(#[props(into)] label: String, value: String) -> Element {
    rsx! {
        div { class: "ow-panel flex flex-col gap-1 rounded-xl p-4",
            span { class: "ow-display text-2xl font-extrabold tabular-nums text-fg", "{value}" }
            span { class: "text-sm text-muted", "{label}" }
        }
    }
}

#[component]
pub fn Stats() -> Element {
    let lib = library();
    if lib.history.is_empty() {
        return rsx! {
            Page { title: t("stats.title"),
                EmptyState { icon: "chart-column", title: t("stats.empty") }
            }
        };
    }
    let totals = logic::totals(&lib.history);
    let (tracks, plays): (Vec<_>, Vec<_>) =
        logic::top_tracks(&lib.history, TOP).into_iter().unzip();
    let plays: Vec<String> = plays.iter().map(u32::to_string).collect();
    let artists = logic::top_artists(&lib.history, TOP);
    rsx! {
        Page { title: t("stats.title"),
            div { class: "mb-8 grid grid-cols-2 gap-3 min-[1100px]:grid-cols-4",
                Tile { label: t("stats.plays"), value: totals.plays.to_string() }
                Tile { label: t("stats.tracks"), value: totals.tracks.to_string() }
                Tile { label: t("stats.artists"), value: totals.artists.to_string() }
                Tile { label: t("stats.skips"), value: totals.skips.to_string() }
            }
            h2 { class: "ow-display mb-3 text-xl font-bold text-fg", {t("stats.top_tracks")} }
            TrackList {
                tracks,
                extra: Some(plays),
                extra_label: t("stats.plays"),
                label: t("stats.top_tracks"),
            }
            h2 { class: "ow-display mb-3 mt-8 text-xl font-bold text-fg", {t("stats.top_artists")} }
            ol { class: "ow-panel flex flex-col rounded-xl p-2",
                for (i, a) in artists.into_iter().enumerate() {
                    li {
                        key: "{a.name}",
                        class: "flex min-h-11 items-center gap-3 rounded-md px-3",
                        span { class: "w-6 text-sm tabular-nums text-muted", "{i + 1}" }
                        if a.id.is_empty() {
                            span { class: "min-w-0 flex-1 truncate text-sm font-semibold text-fg", "{a.name}" }
                        } else {
                            Link {
                                to: Route::Artist { id: a.id.clone() },
                                class: "min-w-0 flex-1 truncate text-sm font-semibold text-fg hover:underline",
                                "{a.name}"
                            }
                        }
                        span { class: "text-sm tabular-nums text-muted", "{a.plays}" }
                    }
                }
            }
        }
    }
}
