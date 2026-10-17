//! What the search page shows before anything is typed: a headline, "Son aramalar" chips and
//! colourful suggestion tiles.

use dioxus::prelude::*;

use super::logic::suggested_searches;
use crate::features::tracks::state::{library, update_library};
use crate::i18n::{language, t, t_args};
use crate::routes::Route;
use crate::ui::{Button, ButtonSize, ButtonVariant, Icon};

/// Suggestion tiles are plain surfaces: the words carry them, not a colour per tile.
const TILE_HUES: [&str; 6] = ["bg-surface"; 6];

fn go(term: &str) {
    navigator().push(Route::Search { q: term.to_owned() });
}

#[component]
fn RecentChip(term: String) -> Element {
    let (open, forget) = (term.clone(), term.clone());
    let remove = t_args("search.recent_remove", &[("term", term.as_str())]);
    rsx! {
        li { class: "group inline-flex h-10 items-center rounded-full border border-line-strong bg-surface transition-colors duration-(--ow-dur-hover) hover:bg-hover",
            button {
                r#type: "button",
                class: "flex h-10 max-w-[260px] items-center gap-2 rounded-full pl-4 pr-1 text-sm font-medium text-fg",
                onclick: move |_| go(&open),
                Icon { name: "clock", size: 16, class: "flex-none text-muted" }
                span { class: "truncate", "{term}" }
            }
            button {
                r#type: "button",
                "aria-label": "{remove}",
                class: "mr-1 flex h-8 w-8 flex-none items-center justify-center rounded-full text-muted hover:bg-press hover:text-fg",
                onclick: move |_| update_library(|l| l.forget_search(&forget)),
                Icon { name: "x", size: 14 }
            }
        }
    }
}

#[component]
fn RecentSearches(terms: Vec<String>) -> Element {
    rsx! {
        section { class: "mb-10", "aria-labelledby": "ow-recent-h",
            div { class: "mb-3 flex items-center justify-between",
                h2 { id: "ow-recent-h", class: "ow-display text-2xl font-extrabold text-fg", {t("search.recent")} }
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    aria_label: t("search.recent_clear"),
                    onclick: move |_| update_library(|l| l.clear_searches()),
                    {t("common.clear")}
                }
            }
            ul { class: "flex flex-wrap gap-2",
                for term in terms {
                    RecentChip { key: "{term}", term }
                }
            }
        }
    }
}

#[component]
fn SuggestionTile(term: String, hue: &'static str) -> Element {
    let target = term.clone();
    rsx! {
        button {
            r#type: "button",
            class: "ow-tile group relative flex h-[88px] items-end justify-between gap-3 overflow-hidden rounded-xl border border-line p-4 text-left text-fg {hue}",
            onclick: move |_| go(&target),
            span { class: "ow-display relative line-clamp-2 text-lg font-bold leading-tight", "{term}" }
            Icon { name: "arrow-right", size: 18, class: "flex-none text-muted transition-colors group-hover:text-accent-text" }
        }
    }
}

#[component]
pub fn IdleView() -> Element {
    let recent = library().recent_searches.clone();
    rsx! {
        div { class: "ow-enter",
            div { class: "mb-10 max-w-[620px]",
                p { class: "ow-display text-[clamp(24px,2.6vw,30px)] font-bold leading-[1.15] tracking-[-0.02em] text-fg",
                    {t("search.empty.title")}
                }
                p { class: "mt-3 text-base text-muted", {t("search.empty.body")} }
            }
            if !recent.is_empty() {
                RecentSearches { terms: recent }
            }
            section { "aria-labelledby": "ow-suggest-h",
                h2 { id: "ow-suggest-h", class: "ow-display mb-3 text-2xl font-extrabold text-fg", {t("search.suggestions")} }
                div { class: "grid grid-cols-3 gap-3 max-[900px]:grid-cols-2",
                    for (i, s) in suggested_searches(language()).into_iter().enumerate() {
                        SuggestionTile { key: "{s}", term: s.to_string(), hue: TILE_HUES[i % TILE_HUES.len()] }
                    }
                }
            }
        }
    }
}
