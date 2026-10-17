//! Cover art and MediaCard.
//!
//! * `Cover { src, alt, shape }`: fixed aspect ratio, lazy-decoded image, music-note fallback when
//!   there is no source or it fails to load. Never shifts layout.
//! * `MediaCard { title, subtitle, cover, on_open, on_play }`: card as wide as `--ow-card-w`, 16px cover radius, title,
//!   subtitle; hover or keyboard focus lifts the card and reveals the gradient play button. Open and play are two separate focusable buttons.

use dioxus::prelude::*;

use super::icon::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum CoverShape {
    /// Rounded square (albums, playlists, tracks): 12px radius, for thumbs.
    #[default]
    Square,
    /// Rounded square with the 16px card radius (media cards, banners).
    Card,
    /// Circle (artists).
    Round,
}

#[component]
pub fn Cover(
    #[props(into, default)] src: Option<String>,
    #[props(into, default)] alt: String,
    #[props(default)] shape: CoverShape,
    /// Fixed edge in px; `None` fills the container width.
    #[props(default)]
    size: Option<u32>,
    #[props(default)] class: String,
) -> Element {
    // Local-file covers are stored as `owcover:<hash>` and served by the loopback proxy.
    let src = src
        .map(|u| crate::backend::api::cover_src(&u))
        .filter(|u| !u.is_empty());
    let mut failed = use_signal(|| false);
    let mut loaded = use_signal(|| false);
    // Reset only when `src` really changes. Resetting on the first run would erase an `onload`
    // that already fired for a cached image, leaving the cover blank forever.
    let mut last_src = use_signal(|| src.clone());
    use_effect(use_reactive!(|src| {
        if *last_src.peek() != src {
            last_src.set(src);
            failed.set(false);
            loaded.set(false);
        }
    }));
    let radius = match shape {
        CoverShape::Square => "rounded-md",
        CoverShape::Card => "rounded-lg",
        CoverShape::Round => "rounded-full",
    };
    let style = size
        .map(|s| format!("width:{s}px;height:{s}px"))
        .unwrap_or_default();
    let fixed = if size.is_some() {
        "flex-none"
    } else {
        "w-full aspect-square"
    };
    let show_img = src.is_some() && !failed();
    rsx! {
        div {
            class: "relative overflow-hidden bg-surface-2 {fixed} {radius} {class}",
            style: "{style}",
            if !show_img || !loaded() {
                div { class: "absolute inset-0 flex items-center justify-center bg-surface-2 text-muted",
                    Icon { name: "music", size: if size.is_some_and(|s| s < 64) { 18 } else { 28 } }
                }
            }
            if let Some(url) = src.filter(|_| show_img) {
                img {
                    src: "{url}",
                    alt: "{alt}",
                    loading: "lazy",
                    decoding: "async",
                    draggable: "false",
                    // Always opaque: a cached image can finish loading before `onload` is attached, which
                    // used to leave it invisible. The placeholder sits behind it and is covered once painted.
                    class: "absolute inset-0 h-full w-full object-cover",
                    onload: move |_| loaded.set(true),
                    onerror: move |_| failed.set(true),
                }
            }
        }
    }
}

#[component]
pub fn MediaCard(
    #[props(into)] title: String,
    #[props(into, default)] subtitle: Option<String>,
    #[props(into, default)] cover: Option<String>,
    #[props(default)] shape: CoverShape,
    /// Accessible name of the play button, e.g. "Çal: {title}".
    #[props(into)]
    play_label: String,
    #[props(default)] on_open: EventHandler<()>,
    #[props(default)] on_play: EventHandler<()>,
    #[props(default)] class: String,
    /// Artwork drawn instead of `cover` (e.g. a 2 x 2 mosaic for a mix).
    #[props(default)]
    art: Option<Element>,
) -> Element {
    let text_align = if shape == CoverShape::Round {
        "text-center"
    } else {
        "text-left"
    };
    rsx! {
        div { class: "ow-card group relative flex w-(--ow-card-w) flex-none flex-col rounded-lg p-2 {class}",
            // The play button sits on the cover's corner whatever the card width.
            div { class: "relative",
                if let Some(art) = art {
                    div { class: "ow-card-cover overflow-hidden rounded-lg", {art} }
                } else {
                    Cover {
                        src: cover,
                        shape: if shape == CoverShape::Square { CoverShape::Card } else { shape },
                        alt: "",
                        class: "ow-card-cover",
                    }
                }
                button {
                    r#type: "button",
                    "aria-label": "{play_label}",
                    class: "ow-accent-fill ow-accent-glow-sm absolute bottom-2 right-2 z-10 flex h-11 w-11 translate-y-2 items-center justify-center rounded-full opacity-0 transition-[opacity,transform,filter] duration-(--ow-dur-hover) ease-ow hover:brightness-110 focus-visible:translate-y-0 focus-visible:opacity-100 group-hover:translate-y-0 group-hover:opacity-100 active:scale-95",
                    onclick: move |_| on_play.call(()),
                    Icon { name: "play", size: 20, filled: true }
                }
            }
            div { class: "mt-3 min-w-0 {text_align}",
                button {
                    r#type: "button",
                    class: "block w-full truncate text-sm font-semibold text-fg outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:outline-2 focus-visible:after:outline-accent-text {text_align}",
                    title: "{title}",
                    onclick: move |_| on_open.call(()),
                    "{title}"
                }
                if let Some(s) = subtitle {
                    p { class: "line-clamp-2 break-words text-sm text-muted", title: "{s}", "{s}" }
                }
            }
        }
    }
}
