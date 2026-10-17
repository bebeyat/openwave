//! Playlist artwork (2 x 2 mosaic) and the playlist card used in the Library grid.

use dioxus::prelude::*;

use super::logic::Mosaic;
use crate::i18n::t;
use crate::ui::Icon;

#[component]
fn Cell(src: String) -> Element {
    let mut failed = use_signal(|| false);
    // Local and custom pictures are stored as `owcover:<hash>`.
    let src = crate::backend::api::cover_src(&src);
    rsx! {
        div { class: "relative h-full w-full overflow-hidden bg-surface-2",
            div { class: "absolute inset-0 flex items-center justify-center text-muted",
                Icon { name: "music", size: 16 }
            }
            if !failed() {
                img {
                    src: "{src}",
                    alt: "",
                    loading: "lazy",
                    decoding: "async",
                    draggable: "false",
                    class: "absolute inset-0 h-full w-full object-cover",
                    onerror: move |_| failed.set(true),
                }
            }
        }
    }
}

/// Square cover for a playlist. `size` is a fixed edge in px; `None` fills the container width.
#[component]
pub fn CoverMosaic(
    mosaic: Mosaic,
    #[props(default)] size: Option<u32>,
    #[props(into, default = "list-music".to_string())] icon: String,
    #[props(default)] class: String,
) -> Element {
    let style = size
        .map(|s| format!("width:{s}px;height:{s}px"))
        .unwrap_or_default();
    let fixed = if size.is_some() {
        "flex-none"
    } else {
        "w-full aspect-square"
    };
    let icon_size = size.map_or(40, |s| (s / 4).clamp(24, 56));
    rsx! {
        div {
            class: "relative overflow-hidden rounded-lg bg-surface-2 {fixed} {class}",
            style: "{style}",
            "aria-hidden": "true",
            match mosaic {
                Mosaic::Empty => rsx! {
                    div { class: "absolute inset-0 flex items-center justify-center text-muted",
                        Icon { name: icon, size: icon_size }
                    }
                },
                Mosaic::Single(url) => rsx! {
                    div { class: "absolute inset-0",
                        Cell { src: url }
                    }
                },
                Mosaic::Grid(cells) => rsx! {
                    div { class: "absolute inset-0 grid grid-cols-2 grid-rows-2 gap-px bg-line",
                        for (i, url) in cells.into_iter().enumerate() {
                            Cell { key: "{i}", src: url }
                        }
                    }
                },
            }
        }
    }
}

/// Library grid card: mosaic cover (hover: hairline + play button), title opens, the round
/// button plays. `liked` swaps the mosaic for the solid accent heart tile of "Beğenilenler".
#[component]
pub fn PlaylistCard(
    #[props(into)] title: String,
    #[props(into)] subtitle: String,
    mosaic: Mosaic,
    #[props(into, default = "list-music".to_string())] icon: String,
    /// Draw the gradient heart tile instead of a mosaic ("Beğenilenler").
    #[props(default)]
    liked: bool,
    #[props(default = true)] can_play: bool,
    #[props(into)] play_label: String,
    on_open: EventHandler<()>,
    on_play: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "ow-card group relative flex min-w-0 flex-col rounded-xl p-2.5",
            div { class: "relative rounded-xl transition-shadow duration-(--ow-dur-hover) ease-ow group-hover:shadow-[0_0_0_1px_var(--ow-border-strong)] group-focus-within:shadow-[0_0_0_1px_var(--ow-border-strong)]",
                if liked {
                    div {
                        class: "ow-accent-fill relative flex aspect-square w-full items-center justify-center overflow-hidden rounded-xl",
                        "aria-hidden": "true",
                        Icon { name: "heart", size: 64, filled: true }
                    }
                } else {
                    CoverMosaic { mosaic, icon, class: "rounded-xl! ring-1 ring-line" }
                }
                if can_play {
                    button {
                        r#type: "button",
                        "aria-label": "{play_label}",
                        class: "ow-accent-fill ow-accent-glow-sm absolute bottom-3 right-3 z-10 flex h-12 w-12 translate-y-2 items-center justify-center rounded-full opacity-0 transition-[opacity,transform,filter] duration-(--ow-dur-hover) ease-ow hover:brightness-110 focus-visible:translate-y-0 focus-visible:opacity-100 group-hover:translate-y-0 group-hover:opacity-100 active:scale-95",
                        onclick: move |_| on_play.call(()),
                        Icon { name: "play", size: 22, filled: true }
                    }
                }
            }
            div { class: "mt-3 min-w-0 px-0.5 text-left",
                button {
                    r#type: "button",
                    class: "ow-display block w-full truncate text-left text-base font-bold text-fg outline-none after:absolute after:inset-0 after:rounded-xl focus-visible:after:outline-2 focus-visible:after:outline-accent-text",
                    title: "{title}",
                    onclick: move |_| on_open.call(()),
                    "{title}"
                }
                p { class: "truncate text-sm text-muted", "{subtitle}" }
            }
        }
    }
}

/// Dashed "new playlist" card that ends the grid.
#[component]
pub fn NewPlaylistCard(on_click: EventHandler<()>) -> Element {
    rsx! {
        div { class: "group relative flex min-w-0 flex-col rounded-xl p-2.5",
            div { class: "flex aspect-square w-full items-center justify-center rounded-xl border-2 border-dashed border-line-strong text-muted transition-colors duration-(--ow-dur-hover) ease-ow group-hover:border-accent group-hover:text-accent-text",
                Icon { name: "plus", size: 36 }
            }
            div { class: "mt-3 min-w-0 px-0.5",
                button {
                    r#type: "button",
                    class: "ow-display block w-full truncate text-left text-base font-bold text-fg outline-none after:absolute after:inset-0 after:rounded-xl focus-visible:after:outline-2 focus-visible:after:outline-accent-text",
                    onclick: move |_| on_click.call(()),
                    {t("library.add.new_playlist")}
                }
            }
        }
    }
}
