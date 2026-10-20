//! Mini player: the whole (small, always-on-top) window shows the cover, the title, the transport
//! and the seek bar. The rest of the app stays mounted underneath, so playback is untouched.

use dioxus::desktop::use_window;
use dioxus::prelude::*;

use super::controls::{SeekRow, Transport};
use crate::features::shell::titlebar::DragRegion;
use crate::features::shell::window::{request_close, toggle_mini, MINI};
use crate::i18n::t;
use crate::player::controller as ctl;
use crate::ui::{CoverThumb, IconButton, IconButtonSize, TooltipAlign, TooltipSide};

#[component]
pub fn MiniPlayer() -> Element {
    if !*MINI.read() {
        return rsx! {};
    }
    rsx! { Surface {} }
}

#[component]
fn Surface() -> Element {
    let window = use_window();
    let w_close = window.clone();
    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    let title = track.as_ref().map(|t| t.title.clone()).unwrap_or_default();
    let artists = track
        .as_ref()
        .map(|t| t.artists_label())
        .unwrap_or_default();
    let src = track
        .as_ref()
        .and_then(|t| t.cover.as_ref())
        .map(|c| c.url.to_string());
    rsx! {
        div {
            class: "fixed inset-0 z-[70] flex flex-col gap-1 rounded-xl border border-line bg-(--ow-bg) px-3 pb-2 pt-2 text-fg",
            role: "region",
            "aria-label": t("player.mini"),
            div { class: "flex min-h-0 items-center gap-3",
                CoverThumb { src, size: 52u32 }
                div { class: "relative min-w-0 flex-1",
                    DragRegion { class: "absolute inset-0" }
                    p { class: "pointer-events-none truncate text-sm font-bold", "{title}" }
                    p { class: "pointer-events-none truncate text-xs text-muted", "{artists}" }
                }
                IconButton {
                    icon: "maximize-2",
                    label: t("player.mini.exit"),
                    shortcut: "P",
                    size: IconButtonSize::Sm,
                    tooltip_side: TooltipSide::Bottom,
                    tooltip_align: TooltipAlign::End,
                    onclick: move |_| toggle_mini(&window),
                }
                IconButton {
                    icon: "x",
                    label: t("window.close"),
                    size: IconButtonSize::Sm,
                    tooltip_side: TooltipSide::Bottom,
                    tooltip_align: TooltipAlign::End,
                    onclick: move |_| request_close(&w_close),
                }
            }
            div { class: "flex justify-center", Transport { modes: false } }
            SeekRow { class: "-mt-1 text-xs" }
        }
    }
}
