//! `DetailHero`: the opening of playlist, album and artist pages.
//!
//! Flat: the artwork tile (or round avatar) next to a large display title, one muted meta line
//! and the actions, bottom-aligned on the page surface and closed by a hairline. No blurred
//! banner or scrim: the window backdrop already carries the cover colour.

use dioxus::prelude::*;

use crate::ui::Icon;

#[component]
pub fn DetailHero(
    /// Cover URL of the page. Kept for callers; the flat hero does not paint it as a backdrop.
    #[props(into, default)]
    backdrop: Option<String>,
    /// Plain title. Ignored when `title_slot` is given.
    #[props(into, default)]
    title: String,
    /// Replaces the `h1` entirely (the playlist title editor).
    #[props(default)]
    title_slot: Option<Element>,
    /// Optional line above the meta (e.g. the album artist link).
    #[props(default)]
    byline: Option<Element>,
    #[props(into, default)] meta: Option<String>,
    /// Sharp artwork next to the title.
    #[props(default)]
    art: Option<Element>,
    /// The artwork is a round avatar (artists): its hairline frame follows the circle.
    #[props(default)]
    round_art: bool,
    #[props(default)] actions: Option<Element>,
) -> Element {
    let _ = backdrop;
    let frame = if round_art {
        "rounded-full"
    } else {
        "rounded-lg"
    };
    rsx! {
        section { class: "ow-detail-hero relative z-20 mb-6 flex items-end gap-7 border-b border-line pb-8 pt-2 max-[1100px]:gap-5",
            if let Some(a) = art {
                div { class: "flex-none {frame} shadow-[0_0_0_1px_var(--ow-border)]", {a} }
            }
            div { class: "min-w-0 flex-1",
                if let Some(slot) = title_slot {
                    {slot}
                } else {
                    h1 { class: "ow-hero-title line-clamp-2 break-words text-fg",
                        title: "{title}",
                        "{title}"
                    }
                }
                if let Some(b) = byline {
                    div { class: "mt-2 text-base font-semibold text-fg", {b} }
                }
                if let Some(m) = meta {
                    p { class: "mt-2 text-sm text-muted", "{m}" }
                }
                if let Some(a) = actions {
                    div { class: "mt-6 flex flex-wrap items-center gap-3", {a} }
                }
            }
        }
    }
}

/// Outlined secondary button for the hero actions ("Karıştır", "Kitaplığıma kaydet").
#[component]
pub fn HeroButton(
    #[props(into)] icon: String,
    #[props(default)] disabled: bool,
    #[props(default)] loading: bool,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            disabled: disabled || loading,
            "aria-busy": loading.then_some("true"),
            class: "inline-flex h-10 items-center gap-2 rounded-md px-4 text-sm font-semibold text-fg ring-1 ring-line-strong transition-[background-color] duration-(--ow-dur-hover) ease-ow hover:bg-hover active:bg-press active:duration-(--ow-dur-press) disabled:pointer-events-none disabled:opacity-50",
            onclick: move |e| onclick.call(e),
            if loading {
                Icon { name: "loader-circle", size: 18, class: "ow-spin" }
            } else {
                Icon { name: icon, size: 18 }
            }
            {children}
        }
    }
}
