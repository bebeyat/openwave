//! Small building blocks shared by the discovery screens: card grids and rows, loading skeletons,
//! the "no results" and error states.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::ui::{Button, ButtonVariant, EmptyState, Icon, Skeleton};

/// Responsive grid of `MediaCard`s: as many columns as fit, cards stretch to fill each row.
#[component]
pub fn CardGrid(children: Element) -> Element {
    rsx! { div { class: "ow-card-grid -mx-2 grid grid-cols-[repeat(auto-fill,minmax(var(--ow-card-w),1fr))] gap-y-2", {children} } }
}

/// One horizontally scrolling row of `MediaCard`s (Home sections).
#[component]
pub fn CardRow(children: Element) -> Element {
    rsx! { div { class: "-mx-2 flex gap-1 overflow-x-auto px-0 pb-2", {children} } }
}

/// Placeholder cards while a grid or row loads.
#[component]
pub fn CardSkeletons(#[props(default = 6)] count: usize) -> Element {
    rsx! {
        div { class: "-mx-2 flex gap-1 overflow-hidden", role: "status", "aria-busy": "true",
            span { class: "sr-only", {t("common.loading")} }
            for i in 0..count {
                div { key: "{i}", class: "flex w-[176px] flex-none flex-col gap-2 p-2",
                    Skeleton { class: "aspect-square w-full" }
                    Skeleton { class: "h-3.5 w-3/4" }
                    Skeleton { class: "h-3 w-1/2" }
                }
            }
        }
    }
}

/// Shown only after a request COMPLETED with zero items. Never render this while loading.
#[component]
pub fn NoResults(#[props(into, default = "search".to_string())] icon: String) -> Element {
    rsx! { EmptyState { icon, title: t("search.no_results"), text: t("search.no_results_hint") } }
}

/// Plain-words error with a retry button.
#[component]
pub fn ErrorState(
    #[props(into)] message: String,
    on_retry: EventHandler<()>,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        EmptyState { icon: "circle-alert", title: message, class,
            Button {
                variant: ButtonVariant::Secondary,
                icon: "refresh-cw",
                onclick: move |_| on_retry.call(()),
                {t("common.retry")}
            }
        }
    }
}

/// Section heading used on Home.
#[component]
pub fn SectionHeading(
    #[props(into)] title: String,
    #[props(into, default)] id: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "mb-3 flex items-center justify-between gap-4",
            h2 { id, class: "text-xl font-bold text-fg", "{title}" }
            {children}
        }
    }
}

/// Cover + kind label + title (the page `h1`) + free-form meta/buttons, for artist and album pages.
#[component]
pub fn DetailHeader(
    #[props(into, default)] cover: Option<String>,
    #[props(default)] round: bool,
    #[props(into)] kind: String,
    #[props(into)] title: String,
    children: Element,
) -> Element {
    let shape = if round {
        crate::ui::CoverShape::Round
    } else {
        crate::ui::CoverShape::Square
    };
    rsx! {
        header { class: "mb-8 flex items-end gap-6 max-[1100px]:gap-4",
            crate::ui::Cover { src: cover, size: 192, alt: "", shape }
            div { class: "min-w-0 flex-1",
                p { class: "text-sm font-semibold text-muted", "{kind}" }
                h1 { class: "mb-2 line-clamp-2 break-words text-2xl font-bold tracking-tight text-fg", "{title}" }
                {children}
            }
        }
    }
}

/// Empty state: a quiet accent-washed icon tile, a title, one line and the actions.
#[component]
pub fn IllustratedEmpty(
    #[props(into)] icon: String,
    #[props(into)] title: String,
    #[props(into, default)] text: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center gap-3 px-6 py-14 text-center",
            span { class: "ow-chip-icon mb-2 h-16 w-16 rounded-2xl", "aria-hidden": "true",
                Icon { name: icon, size: 28 }
            }
            h3 { class: "ow-display text-xl font-bold text-fg", "{title}" }
            if let Some(t) = text {
                p { class: "max-w-[420px] text-sm text-muted", "{t}" }
            }
            div { class: "mt-2 flex flex-wrap items-center justify-center gap-2", {children} }
        }
    }
}
