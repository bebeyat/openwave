//! Fullscreen player: blurred cover backdrop, a large rounded card with the cover, title, gradient
//! seek bar, transport and the lyrics as giant type on the right.
//!
//! It covers the content and the player bar but not the top row, so the window buttons stay
//! reachable. The backdrop is one static layer (the same blurred cover the app backdrop uses,
//! skipped in flat mode); motion follows the global reduced-motion / low-power tokens.

use dioxus::prelude::*;

use super::controls::{SeekRow, Transport, VolumeControl};
use super::effects_popover::FxButton;
use super::lyrics::LyricsPane;
use super::state::{
    is_current_liked, toggle_drawer, toggle_like_current, DrawerTab, DRAWER, FULLSCREEN,
    FULLSCREEN_LYRICS, FULLSCREEN_VIZ,
};
use crate::features::shell::titlebar::{DragRegion, WindowControls};
use crate::i18n::t;
use crate::player::controller as ctl;
use crate::routes::Route;
use crate::ui::{
    Cover, CoverShape, CoverThumb, DropdownMenu, Icon, IconButton, MenuAlign, MenuItem, Tooltip,
    TooltipAlign, TooltipSide,
};

const COLLAPSE_ID: &str = "ow-fs-collapse";
const FULLSCREEN_BUTTON_ID: &str = "ow-fullscreen-btn";

/// Dark token overrides: everything inside renders as in the dark theme, whatever the app theme.
const DARK_TOKENS: &str = "--ow-text:#f2f2f2;--ow-muted:#d0d0d0;--ow-bg:#121212;--ow-surface:#1c1c1e;--ow-surface-2:rgba(255,255,255,.12);\
--ow-border:rgba(255,255,255,.18);--ow-border-strong:rgba(255,255,255,.6);--ow-hover:rgba(255,255,255,.12);--ow-press:rgba(255,255,255,.18);\
--ow-accent-text:var(--ow-at-d);--ow-glass:rgba(26,24,36,.86);--ow-scrim:rgba(0,0,0,.6);color-scheme:dark";

#[component]
pub fn FullscreenPlayer() -> Element {
    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    let open = *FULLSCREEN.read();
    use_effect(use_reactive!(|open, track| {
        if open && track.is_none() {
            *FULLSCREEN.write() = false;
        }
    }));
    match (open, track) {
        (true, Some(track)) => rsx! { Surface { track } },
        _ => rsx! {},
    }
}

#[component]
fn Surface(track: crate::model::Track) -> Element {
    use_effect(|| {
        document::eval(&format!("window.ow.focusId('{COLLAPSE_ID}')"));
    });
    use_drop(|| {
        document::eval(&format!(
            "setTimeout(function(){{var a=document.activeElement;if(!a||a===document.body){{var e=document.getElementById('{FULLSCREEN_BUTTON_ID}');if(e)e.focus();}}}},60);"
        ));
    });
    let src = track.cover.as_ref().map(|c| c.url.to_string());
    let lyrics_on = *FULLSCREEN_LYRICS.read();
    let queue_open = *DRAWER.read() == Some(DrawerTab::Queue);
    rsx! {
        div {
            class: "ow-fade-in ow-fs-bg fixed inset-2 z-30 overflow-hidden rounded-2xl text-fg",
            style: "{DARK_TOKENS}",
            role: "region",
            "aria-label": t("player.fullscreen"),
            if let Some(url) = src.clone() {
                img {
                    src: "{url}",
                    alt: "",
                    "aria-hidden": "true",
                    class: "ow-fs-img pointer-events-none absolute inset-0 h-full w-full object-cover opacity-80",
                }
            }
            div { class: "absolute inset-0 bg-[rgba(8,7,12,var(--ow-sa-d,.5))]" }
            // The overlay hides the shell, so it carries its own drag strip and window buttons.
            div { class: "absolute inset-x-0 top-0 z-10 flex h-14 items-center pl-4 pr-1",
                DragRegion { class: "h-full min-w-6 flex-1" }
                WindowControls {}
            }
            div { class: "relative flex h-full flex-col px-4 pb-4 pt-16",
                TopRow { lyrics_on, queue_open }
                div { class: "mx-auto my-auto flex min-h-0 w-full max-w-[1080px] flex-col gap-4 rounded-2xl border border-line bg-[rgba(18,18,17,.92)] p-6 min-[1100px]:p-8",
                    div { class: if lyrics_on { "flex min-h-0 flex-1 gap-8 min-[1100px]:gap-12" } else { "flex min-h-0 flex-1 justify-center" },
                        // The cover shrinks with the window height so the card never scrolls.
                        div { class: "flex min-w-0 flex-none flex-col gap-4 w-[clamp(168px,calc(100vh-440px),var(--ow-fs-art))]",
                            Cover {
                                src,
                                alt: "",
                                shape: CoverShape::Card,
                                class: "shadow-[0_0_0_1px_rgba(255,255,255,.1)]",
                            }
                            TitleBlock { track: track.clone() }
                        }
                        if lyrics_on {
                            div { class: "relative min-h-0 min-w-0 flex-1",
                                div { class: "absolute inset-0",
                                    LyricsPane { large: true, on_dark: true }
                                }
                            }
                        }
                    }
                    if *FULLSCREEN_VIZ.read() {
                        Visualizer {}
                    }
                    SeekRow {}
                    div { class: "grid grid-cols-[1fr_auto_1fr] items-center gap-4",
                        div { class: "flex items-center", VolumeControl { slider_class: "w-24" } }
                        Transport { big: true }
                        NextCard {}
                    }
                }
            }
        }
    }
}

#[component]
fn TopRow(lyrics_on: bool, queue_open: bool) -> Element {
    rsx! {
        div { class: "flex flex-none items-center justify-between",
            IconButton {
                id: COLLAPSE_ID,
                icon: "chevron-down",
                label: t("player.exit_fullscreen"),
                shortcut: "Esc",
                tooltip_side: TooltipSide::Bottom,
                tooltip_align: TooltipAlign::Start,
                onclick: move |_| *FULLSCREEN.write() = false,
            }
            div { class: "flex items-center gap-1",
                FxButton {
                    id: "ow-fx-btn-fs",
                    tooltip_side: TooltipSide::Bottom,
                    tooltip_align: TooltipAlign::End,
                }
                PaneToggle {
                    icon: "audio-lines",
                    label: t("player.visualizer"),
                    shortcut: "V",
                    pressed: *FULLSCREEN_VIZ.read(),
                    onclick: move |_| {
                        let on = !*FULLSCREEN_VIZ.peek();
                        *FULLSCREEN_VIZ.write() = on;
                    },
                }
                PaneToggle {
                    icon: "mic-vocal",
                    label: t("player.lyrics"),
                    shortcut: "L",
                    pressed: lyrics_on,
                    onclick: move |_| {
                        let on = !*FULLSCREEN_LYRICS.peek();
                        *FULLSCREEN_LYRICS.write() = on;
                    },
                }
                PaneToggle {
                    icon: "list-music",
                    label: t("player.queue"),
                    shortcut: "Q",
                    pressed: queue_open,
                    onclick: move |_| toggle_drawer(DrawerTab::Queue),
                }
            }
        }
    }
}

/// Frequency bars drawn by `assets/fx.js` into the canvas while it is mounted.
#[component]
fn Visualizer() -> Element {
    use_effect(|| {
        document::eval("window.owFx && window.owFx.viz(true)");
    });
    use_drop(|| {
        document::eval("window.owFx && window.owFx.viz(false)");
    });
    rsx! {
        canvas { id: "ow-viz", "aria-hidden": "true", class: "h-14 w-full flex-none text-accent-text opacity-80" }
    }
}

/// Title, artist and the ⋯ menu under the cover.
#[component]
fn TitleBlock(track: crate::model::Track) -> Element {
    let liked = is_current_liked();
    let artist = track
        .artists
        .iter()
        .find(|a| !a.id.is_empty())
        .map(|a| a.id.to_string());
    let artists = track.artists_label();
    let like_label = if liked {
        t("player.unlike")
    } else {
        t("player.like")
    };
    rsx! {
        div { class: "flex items-start gap-2",
            div { class: "min-w-0 flex-1",
                h2 {
                    class: "ow-display truncate text-2xl font-extrabold",
                    title: "{track.title}",
                    "{track.title}"
                }
                p { class: "truncate text-base text-muted", "{artists}" }
            }
            DropdownMenu { icon: "ellipsis", aria_label: t("common.more"), align: MenuAlign::End,
                MenuItem {
                    icon: "heart",
                    label: like_label,
                    onselect: move |_| toggle_like_current(),
                }
                if let Some(id) = artist {
                    MenuItem {
                        icon: "user",
                        label: t("track.go_to_artist"),
                        onselect: move |_| {
                            *FULLSCREEN.write() = false;
                            navigator().push(Route::Artist { id: id.clone() });
                        },
                    }
                }
            }
        }
    }
}

/// The small "Sıradaki" card at the bottom right.
#[component]
fn NextCard() -> Element {
    let next = ctl::QUEUE.read().upcoming().first().cloned();
    let Some(next) = next else {
        return rsx! { div {} };
    };
    let artists = next.track.artists_label();
    let src = next.track.cover.as_ref().map(|c| c.url.to_string());
    let uid = next.uid;
    let label = format!("{}: {}", t("player.next_up"), next.track.title);
    rsx! {
        button {
            r#type: "button",
            class: "ml-auto flex w-[220px] max-w-full items-center gap-3 rounded-md border border-line bg-hover p-2 text-left transition-colors duration-(--ow-dur-hover) hover:bg-press",
            "aria-label": "{label}",
            onclick: move |_| ctl::play_uid(uid),
            CoverThumb { src, size: 44u32 }
            div { class: "min-w-0 flex-1",
                p { class: "text-xs font-semibold text-accent-text", {t("player.next_up")} }
                p { class: "truncate text-sm font-bold text-fg", "{next.track.title}" }
                p { class: "truncate text-xs text-muted", "{artists}" }
            }
        }
    }
}

#[component]
fn PaneToggle(
    icon: String,
    label: String,
    shortcut: String,
    pressed: bool,
    onclick: EventHandler<()>,
) -> Element {
    let tone = if pressed {
        "bg-hover text-accent-text"
    } else {
        "text-fg hover:bg-hover"
    };
    rsx! {
        Tooltip { label: label.clone(), shortcut: shortcut.clone(), side: TooltipSide::Bottom, align: TooltipAlign::End,
            button {
                r#type: "button",
                class: "relative inline-flex h-9 items-center gap-1.5 rounded-full px-3 text-sm font-semibold transition-colors duration-(--ow-dur-hover) active:bg-press {tone}",
                "aria-pressed": "{pressed}",
                "aria-keyshortcuts": "{shortcut}",
                onclick: move |_| onclick.call(()),
                Icon { name: icon, size: 18 }
                "{label}"
                if pressed {
                    span {
                        class: "absolute bottom-0.5 left-1/2 h-1 w-1 -translate-x-1/2 rounded-full bg-accent-text",
                        "aria-hidden": "true",
                    }
                }
            }
        }
    }
}
