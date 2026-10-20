//! The sticky 80px player bar.

use dioxus::prelude::*;

use super::controls::{SeekRow, Transport, VolumeControl};
use super::effects_popover::FxButton;
use super::shortcuts_popover::ShortcutsPopover;
use super::sleep_popover::{SleepButton, SleepPopover};
use super::state::{
    is_current_liked, toggle_drawer, toggle_fullscreen, toggle_like_current, DrawerTab, DRAWER,
    LYRICS_BUTTON_ID, QUEUE_BUTTON_ID, SHORTCUTS_OPEN,
};
use crate::i18n::t;
use crate::model::Track;
use crate::player::controller::{self as ctl, Status};
use crate::routes::Route;
use crate::ui::{CoverThumb, Icon, IconButton, IconButtonSize, Tooltip, TooltipAlign};

#[component]
pub fn PlayerBar() -> Element {
    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    rsx! {
        footer {
            class: "ow-panel relative z-20 flex h-[88px] flex-none items-center rounded-xl px-4 min-[1100px]:px-6",
            "aria-label": t("player.region"),
            "data-slot": "player-bar",
            match track {
                Some(track) => rsx! {
                    div { class: "grid w-full grid-cols-[minmax(0,1.3fr)_minmax(240px,1fr)_auto] min-[1100px]:grid-cols-[minmax(0,1fr)_minmax(240px,1.5fr)_auto] items-center gap-3",
                        TrackInfo { track }
                        // 40px transport + 6px gap + seek row trimmed to 16px (its 28px hit area
                        // overflows into the margins) = 62px, so the space above the play button
                        // matches the space below the seek bar.
                        div { class: "flex w-full min-w-0 max-w-[560px] flex-col items-center gap-1.5 justify-self-center",
                            Transport { modes: crate::features::settings::boot::CUSTOM.read().bar_shuffle_repeat }
                            SeekRow { class: "-my-1.5" }
                        }
                        RightControls {}
                    }
                },
                None => rsx! {
                    div { class: "flex w-full items-center justify-between gap-4",
                        p { class: "text-sm text-muted", {t("player.placeholder")} }
                        ShortcutsButton {}
                    }
                },
            }
            ShortcutsPopover {}
            SleepPopover {}
        }
    }
}

#[component]
fn ArtistLinks(track: Track, #[props(default)] class: String) -> Element {
    let count = track.artists.len();
    rsx! {
        span { class: "min-w-0 truncate {class}",
            if count == 0 {
                {if track.is_local() { t("common.local_file") } else { t("common.unknown_artist") }}
            }
            for (i , artist) in track.artists.iter().enumerate() {
                if i > 0 {
                    ", "
                }
                if artist.id.is_empty() {
                    "{artist.name}"
                } else {
                    Link {
                        to: Route::Artist {
                            id: artist.id.to_string(),
                        },
                        class: "rounded-sm hover:text-fg hover:underline",
                        "{artist.name}"
                    }
                }
            }
        }
    }
}

#[component]
fn TrackInfo(track: Track) -> Element {
    let status = *ctl::STATUS.read();
    let hint = match status {
        Status::Loading => Some(t("player.loading")),
        Status::FindingSimilar => Some(t("player.autoplay_loading")),
        _ => None,
    };
    let liked = is_current_liked();
    let (like_label, like_icon) = if liked {
        (t("player.unlike"), "heart")
    } else {
        (t("player.like"), "heart")
    };
    let src = track.cover.as_ref().map(|c| c.url.to_string());
    // The cover is the one focusable element of the left block: its tooltip carries the full
    // title (and artist) for keyboard users, since the visible text truncates.
    let cover_label = format!("{}: {}", t("player.fullscreen"), track.title);
    let cover_tip = format!("{} · {}", track.title, track.artists_label());
    rsx! {
        div { class: "flex min-w-0 items-center gap-3",
            Tooltip { label: cover_tip, shortcut: "F", align: TooltipAlign::Start,
                button {
                    r#type: "button",
                    class: "block flex-none rounded-md transition-transform duration-(--ow-dur-hover) hover:scale-[1.04] active:scale-95",
                    "aria-label": cover_label,
                    "aria-keyshortcuts": "F",
                    onclick: move |_| toggle_fullscreen(),
                    CoverThumb { src, size: 56u32, class: "ow-cover-glow" }
                }
            }
            div { class: "min-w-0 flex-1",
                p { class: "ow-display truncate text-sm font-bold text-fg", title: "{track.title}", "{track.title}" }
                if let Some(h) = hint {
                    p { class: "truncate text-xs text-muted", role: "status", "{h}" }
                } else {
                    p { class: "flex truncate text-xs text-muted",
                        ArtistLinks { track: track.clone() }
                    }
                }
            }
            if crate::features::settings::boot::CUSTOM.read().bar_like {
            IconButton {
                icon: like_icon,
                label: like_label,
                shortcut: "Ctrl+L",
                size: IconButtonSize::Sm,
                pressed: Some(liked),
                filled: liked,
                class: if liked { "text-accent-text" } else { "" },
                onclick: move |_| toggle_like_current(),
            }
            }
        }
    }
}

#[component]
fn ToggleChip(
    id: String,
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
        Tooltip { label: label.clone(), shortcut: shortcut.clone(),
            button {
                id,
                r#type: "button",
                class: "relative inline-flex h-9 flex-none items-center gap-1.5 rounded-full px-3 text-sm font-semibold transition-colors duration-(--ow-dur-hover) active:bg-press {tone}",
                "aria-pressed": "{pressed}",
                "aria-keyshortcuts": "{shortcut}",
                onclick: move |_| onclick.call(()),
                Icon { name: icon, size: 18 }
                "{label}"
                if pressed {
                    // Second cue besides the accent color: a dot under the label.
                    span {
                        class: "absolute bottom-0.5 left-1/2 h-1 w-1 -translate-x-1/2 rounded-full bg-accent-text",
                        "aria-hidden": "true",
                    }
                }
            }
        }
    }
}

#[component]
fn ShortcutsButton() -> Element {
    rsx! {
        IconButton {
            id: "ow-shortcuts-btn",
            icon: "circle-help",
            label: t("player.shortcuts"),
            size: IconButtonSize::Sm,
            expanded: *SHORTCUTS_OPEN.read(),
            haspopup: "dialog",
            tooltip_align: TooltipAlign::End,
            onclick: move |_| {
                let open = !*SHORTCUTS_OPEN.peek();
                *SHORTCUTS_OPEN.write() = open;
            },
        }
    }
}

#[component]
fn RightControls() -> Element {
    let drawer = *DRAWER.read();
    let c = crate::features::settings::boot::CUSTOM.read().clone();
    rsx! {
        div { class: "flex items-center justify-end gap-1",
            if c.bar_lyrics {
            ToggleChip {
                id: LYRICS_BUTTON_ID,
                icon: "mic-vocal",
                label: t("player.lyrics"),
                shortcut: "L",
                pressed: drawer == Some(DrawerTab::Lyrics),
                onclick: move |_| toggle_drawer(DrawerTab::Lyrics),
            }
            }
            if c.bar_queue {
            ToggleChip {
                id: QUEUE_BUTTON_ID,
                icon: "list-music",
                label: t("player.queue"),
                shortcut: "Q",
                pressed: drawer == Some(DrawerTab::Queue),
                onclick: move |_| toggle_drawer(DrawerTab::Queue),
            }
            }
            if c.bar_effects {
                FxButton { tooltip_align: TooltipAlign::End }
            }
            if c.bar_volume {
                VolumeControl { slider_class: "w-16 min-[1100px]:w-24" }
            }
            // The cover opens fullscreen too, so this button can give way on narrow windows.
            if c.bar_fullscreen {
            div { class: "hidden min-[1100px]:block",
                IconButton {
                    id: "ow-fullscreen-btn",
                    icon: "maximize-2",
                    label: t("player.fullscreen"),
                    shortcut: "F",
                    size: IconButtonSize::Sm,
                    onclick: move |_| toggle_fullscreen(),
                }
            }
            }
            if c.bar_sleep {
                SleepButton {}
            }
            if c.bar_help {
                ShortcutsButton {}
            }
        }
    }
}
