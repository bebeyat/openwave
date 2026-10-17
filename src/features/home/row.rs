//! Home row building blocks: a display-type heading with scroll arrows and a snapping,
//! edge-faded horizontal row of cards.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::ui::{IconButton, IconButtonSize, Skeleton};

/// Fraction of the visible width one arrow press scrolls.
const SCROLL_STEP: f64 = 0.85;

fn scroll_row(id: &str, direction: i32) {
    document::eval(&format!(
        "(function(){{var e=document.getElementById('{id}');if(e)e.scrollBy({{left:{direction}*e.clientWidth*{SCROLL_STEP},behavior:'smooth'}});}})()"
    ));
}

/// Section heading in the display face. `row` (the scroller id) adds the two arrow buttons;
/// children (a retry button, for instance) sit before them.
#[component]
pub fn RowHeading(
    #[props(into)] title: String,
    #[props(into)] id: String,
    #[props(into, default)] row: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "mb-2 flex items-center justify-between gap-4",
            h2 { id, class: "ow-display text-2xl font-extrabold text-fg", "{title}" }
            div { class: "flex items-center gap-1",
                {children}
                if let Some(row) = row {
                    {
                        let (back, forward) = (row.clone(), row);
                        rsx! {
                            IconButton {
                                icon: "chevron-left",
                                label: t("home.scroll_back"),
                                size: IconButtonSize::Sm,
                                class: "max-[900px]:hidden",
                                onclick: move |_| scroll_row(&back, -1),
                            }
                            IconButton {
                                icon: "chevron-right",
                                label: t("home.scroll_forward"),
                                size: IconButtonSize::Sm,
                                class: "max-[900px]:hidden",
                                onclick: move |_| scroll_row(&forward, 1),
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A horizontally scrolling row of cards: scroll-snap, hidden scrollbar, faded edges.
#[component]
pub fn ScrollRow(#[props(into)] id: String, children: Element) -> Element {
    rsx! {
        div { id, class: "ow-snap-row -mb-6 -mt-2 mx-[calc(var(--ow-gutter)*-1)] flex gap-1 overflow-x-auto px-[calc(var(--ow-gutter)-8px)] pb-8 pt-2", {children} }
    }
}

/// Placeholder cards for a loading row, matching the card size.
#[component]
pub fn RowSkeleton(#[props(default = 6)] count: usize, #[props(default)] round: bool) -> Element {
    let shape = if round { "rounded-full" } else { "rounded-lg" };
    rsx! {
        div { class: "-mx-2 flex gap-1 overflow-hidden", role: "status", "aria-busy": "true",
            span { class: "sr-only", {t("common.loading")} }
            for i in 0..count {
                div { key: "{i}", class: "flex w-(--ow-card-w) flex-none flex-col gap-2 p-2",
                    Skeleton { class: "aspect-square w-full !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)] {shape}" }
                    Skeleton { class: "h-3.5 w-3/4 !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)]" }
                    Skeleton { class: "h-3 w-1/2 !bg-[color-mix(in_srgb,var(--ow-text)_9%,transparent)]" }
                }
            }
        }
    }
}
