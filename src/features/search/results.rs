//! The results area of the search page for one tab.

use dioxus::prelude::*;

use super::logic::{Phase, Tab};
use crate::features::tracks::actions::{open_album, open_artist, play_artist, play_collection};
use crate::features::tracks::common::{CardGrid, ErrorState, NoResults};
use crate::features::tracks::TrackList;
use crate::i18n::{t, t_args, t_count};
use crate::model::SearchResults;
use crate::ui::{CoverShape, MediaCard, Skeleton};

fn play_label(title: &str) -> String {
    t_args("home.play_card", &[("title", title)])
}

fn cover_url(c: &Option<crate::model::Cover>) -> Option<String> {
    c.as_ref().map(|c| c.url.to_string())
}

#[component]
fn Cards(tab: Tab, results: std::sync::Arc<SearchResults>) -> Element {
    rsx! {
        CardGrid {
            match tab {
                Tab::Artists => rsx! {
                    for a in results.artists.iter() {
                        {
                            let (open_id, play_id) = (a.id.to_string(), a.id.to_string());
                            rsx! {
                                MediaCard {
                                    key: "{a.id}",
                                    title: a.name.to_string(),
                                    subtitle: t("detail.artist"),
                                    cover: cover_url(&a.cover),
                                    shape: CoverShape::Round,
                                    play_label: play_label(&a.name),
                                    on_open: move |_| open_artist(&open_id),
                                    on_play: move |_| play_artist(play_id.clone()),
                                }
                            }
                        }
                    }
                },
                Tab::Albums => rsx! {
                    for a in results.albums.iter() {
                        {
                            let (open_id, play_id) = (a.id.to_string(), a.id.to_string());
                            let artists = a.artists.iter().map(|x| x.name.as_str()).collect::<Vec<_>>().join(", ");
                            let subtitle = match a.year {
                                Some(y) if !artists.is_empty() => format!("{artists} · {y}"),
                                Some(y) => y.to_string(),
                                None => artists,
                            };
                            rsx! {
                                MediaCard {
                                    key: "{a.id}",
                                    title: a.title.to_string(),
                                    subtitle,
                                    cover: cover_url(&a.cover),
                                    play_label: play_label(&a.title),
                                    on_open: move |_| open_album(&open_id),
                                    on_play: move |_| play_collection(play_id.clone()),
                                }
                            }
                        }
                    }
                },
                _ => rsx! {
                    for p in results.playlists.iter() {
                        {
                            let (open_id, play_id) = (p.id.to_string(), p.id.to_string());
                            rsx! {
                                MediaCard {
                                    key: "{p.id}",
                                    title: p.title.to_string(),
                                    subtitle: p.author.as_ref().map(|a| a.to_string()),
                                    cover: cover_url(&p.cover),
                                    play_label: play_label(&p.title),
                                    on_open: move |_| open_album(&open_id),
                                    on_play: move |_| play_collection(play_id.clone()),
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

/// Loading placeholder for the songs tab: rows shaped like the real ones.
#[component]
fn SongSkeletons() -> Element {
    rsx! {
        div { class: "flex flex-col gap-1",
            for i in 0..8 {
                div { key: "{i}", class: "flex h-14 items-center gap-3 rounded-lg px-3",
                    Skeleton { class: "h-10 w-10 flex-none rounded-md !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)]" }
                    div { class: "flex flex-1 flex-col gap-2",
                        Skeleton { class: "h-3.5 w-1/3" }
                        Skeleton { class: "h-3 w-1/5" }
                    }
                    Skeleton { class: "h-3 w-10" }
                }
            }
        }
    }
}

/// Loading placeholder for the card tabs; round for artists.
#[component]
fn CardPlaceholders(round: bool) -> Element {
    let shape = if round { "rounded-full" } else { "rounded-lg" };
    rsx! {
        CardGrid {
            for i in 0..8 {
                div { key: "{i}", class: "flex w-(--ow-card-w) flex-none flex-col gap-2 p-2",
                    Skeleton { class: "aspect-square w-full !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)] {shape}" }
                    Skeleton { class: "h-3.5 w-3/4 !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)]" }
                    Skeleton { class: "h-3 w-1/2 !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)]" }
                }
            }
        }
    }
}

/// What a screen reader hears for the current phase; empty while there is nothing to say.
/// "No results" is announced only after a request completed with zero items.
pub fn status_text(tab: Tab, phase: &Phase) -> String {
    match phase {
        Phase::Idle | Phase::Failed(_) => String::new(),
        Phase::Loading => t("search.status.searching"),
        Phase::Done(r) => match tab.count(r) {
            0 => t("search.no_results"),
            n => t_count("search.status.count", n),
        },
    }
}

/// Visually hidden live region; keep it mounted so changes are announced.
#[component]
pub fn SearchStatus(tab: Tab, phase: Phase) -> Element {
    rsx! { p { class: "sr-only", role: "status", {status_text(tab, &phase)} } }
}

#[component]
pub fn SearchResultsView(tab: Tab, phase: Phase, on_retry: EventHandler<()>) -> Element {
    let busy = matches!(phase, Phase::Loading);
    rsx! {
        div { "aria-busy": "{busy}", ResultsBody { tab, phase, on_retry } }
    }
}

#[component]
fn ResultsBody(tab: Tab, phase: Phase, on_retry: EventHandler<()>) -> Element {
    // The only place "Sonuç bulunamadı" can appear: a completed request with zero items.
    if phase.shows_no_results(tab) {
        return rsx! { NoResults {} };
    }
    match phase {
        Phase::Idle => rsx! {},
        // Skeletons are decorative here: `SearchStatus` already says "Aranıyor…".
        Phase::Loading => rsx! {
            div { "aria-hidden": "true",
                match tab {
                    Tab::Songs => rsx! { SongSkeletons {} },
                    _ => rsx! { CardPlaceholders { round: tab == Tab::Artists } },
                }
            }
        },
        Phase::Failed(message) => rsx! { ErrorState { message, on_retry } },
        Phase::Done(results) => match tab {
            Tab::Songs => rsx! {
                TrackList { tracks: results.tracks.clone(), label: t("search.tab.songs"), similar_after: true }
            },
            _ => rsx! { Cards { tab, results } },
        },
    }
}
