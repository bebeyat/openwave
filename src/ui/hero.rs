//! Building blocks for detail pages: `GradientPlayButton`, `CoverThumb`.
//!
//! * `GradientPlayButton { playing, label, onclick }`: the one primary round action, a solid
//!   accent disc (the name is historical: no gradient, no glow). `size` picks 48px (`Md`) or
//!   56px (`Lg`, default).
//! * `CoverThumb { src, size }`: fixed-size rounded cover with the neutral fallback tile.

use dioxus::prelude::*;

use super::icon::Icon;
use super::media::{Cover, CoverShape};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum PlaySize {
    /// 48px.
    Md,
    /// 56px.
    #[default]
    Lg,
}

#[component]
pub fn GradientPlayButton(
    /// Show the pause glyph (and `aria-pressed`) while playing.
    #[props(default)]
    playing: bool,
    /// Accessible name, e.g. "Çal" / "Duraklat".
    #[props(into)]
    label: String,
    #[props(default)] size: PlaySize,
    #[props(default)] disabled: bool,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    #[props(into, default)] id: Option<String>,
    #[props(default)] class: String,
) -> Element {
    let (box_class, icon_px) = match size {
        PlaySize::Md => ("h-12 w-12", 22),
        PlaySize::Lg => ("h-14 w-14", 24),
    };
    rsx! {
        button {
            id,
            r#type: "button",
            class: "ow-accent-fill ow-play-ring inline-flex flex-none items-center justify-center rounded-full transition-[transform,filter] duration-(--ow-dur-hover) ease-ow hover:brightness-110 active:scale-95 active:duration-(--ow-dur-press) disabled:pointer-events-none disabled:opacity-50 {box_class} {class}",
            disabled,
            "aria-label": "{label}",
            "aria-pressed": playing.then_some("true"),
            onclick: move |e| onclick.call(e),
            Icon { name: if playing { "pause" } else { "play" }, size: icon_px, filled: true }
        }
    }
}

#[component]
pub fn CoverThumb(
    #[props(into, default)] src: Option<String>,
    #[props(default = 48)] size: u32,
    #[props(default)] shape: CoverShape,
    #[props(into, default)] alt: String,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        Cover { src, alt, shape, size, class: "shadow-[0_0_0_1px_var(--ow-border)] {class}" }
    }
}
