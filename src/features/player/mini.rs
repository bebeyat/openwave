//! Mini player: the whole (small, always-on-top) window shows the cover, the title, the transport
//! and the seek bar. The rest of the app stays mounted underneath, so playback is untouched.
//!
//! When the pointer has been away for [`IDLE_AFTER`] and the song has synced lyrics, the transport
//! fades out for the current lyric line and the seek bar thins to a progress line; the pointer
//! coming back reverses it.

use std::time::Duration;

use dioxus::desktop::use_window;
use dioxus::prelude::*;

use super::controls::{SeekRow, Transport};
use super::lyric_logic::{active_line, shifted};
use super::lyrics_state::{self, LyricsView, LYRICS};
use crate::features::shell::titlebar::DragRegion;
use crate::features::shell::window::{request_close, toggle_mini, MINI};
use crate::features::tracks::state::library;
use crate::i18n::t;
use crate::model::Lyrics;
use crate::player::controller::{self as ctl, POSITION};
use crate::ui::{CoverThumb, IconButton, IconButtonSize, TooltipAlign, TooltipSide};

const IDLE_AFTER: Duration = Duration::from_secs(3);
/// Shared by both layers of each swap; motion-reduce turns the animation off.
const FADE: &str = "transition-all duration-300 ease-out motion-reduce:transition-none";

/// The line to show at `position` (seconds, offset already applied): `None` without synced
/// lyrics, a note sign before the first line and on empty (instrumental) lines.
pub fn current_line(lyrics: &Lyrics, position: f64) -> Option<String> {
    if !lyrics.is_synced() {
        return None;
    }
    let text = active_line(&lyrics.synced, position)
        .map(|i| lyrics.synced[i].text.trim().to_owned())
        .unwrap_or_default();
    Some(if text.is_empty() { "♪".into() } else { text })
}

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
    let mut hover = use_signal(|| false);
    let mut idle = use_signal(|| false);
    // Bumped on every enter/leave so only the latest leave can turn the controls off.
    let mut gen = use_signal(|| 0u64);
    let mut leave_later = move || {
        let mine = gen() + 1;
        gen.set(mine);
        spawn(async move {
            tokio::time::sleep(IDLE_AFTER).await;
            if gen() == mine && !hover() {
                idle.set(true);
            }
        });
    };
    // The pointer may already be elsewhere when the mini player opens.
    use_hook(move || leave_later());

    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    let track_id = track.as_ref().map(|t| t.id.clone());
    let request_track = track.clone();
    use_effect(use_reactive!(|track_id| {
        let _ = track_id;
        if let Some(t) = &request_track {
            lyrics_state::request(t, false);
        }
    }));
    let line = match (&LYRICS.read().view, &track) {
        (LyricsView::Ready(l), Some(t)) if LYRICS.read().track_id.as_ref() == Some(&t.id) => {
            let offset = library().lyric_offset(&t.id);
            current_line(l, shifted(*POSITION.read(), offset))
        }
        _ => None,
    };
    let quiet = idle() && line.is_some();

    let title = track.as_ref().map(|t| t.title.clone()).unwrap_or_default();
    let artists = track
        .as_ref()
        .map(|t| t.artists_label())
        .unwrap_or_default();
    let src = track
        .as_ref()
        .and_then(|t| t.cover.as_ref())
        .map(|c| c.url.to_string());
    let pos = *POSITION.read();
    let dur = ctl::display_duration();
    let pct = if dur > 0.0 {
        (pos / dur * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    let layer = |on: bool, off_extra: &str| {
        if on {
            format!("{FADE} opacity-100 translate-y-0 scale-100")
        } else {
            format!("{FADE} pointer-events-none opacity-0 {off_extra}")
        }
    };
    let (controls, lyric) = (layer(!quiet, "scale-95"), layer(quiet, "translate-y-1"));
    let (seek, thin) = (layer(!quiet, ""), layer(quiet, ""));
    rsx! {
        div {
            class: "fixed inset-0 z-[70] flex flex-col gap-1 rounded-xl border border-line bg-(--ow-bg) px-3 pb-2 pt-2 text-fg",
            role: "region",
            "aria-label": t("player.mini"),
            onmouseenter: move |_| {
                hover.set(true);
                idle.set(false);
                gen += 1;
            },
            onmousemove: move |_| {
                if !hover() {
                    hover.set(true);
                    idle.set(false);
                    gen += 1;
                }
            },
            onmouseleave: move |_| {
                hover.set(false);
                leave_later();
            },
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
            div { class: "relative h-12",
                div {
                    class: "absolute inset-0 flex items-center justify-center {controls}",
                    "aria-hidden": quiet.then_some("true"),
                    Transport { modes: false }
                }
                p {
                    class: "absolute inset-0 flex items-center justify-center px-2 text-center text-sm font-semibold leading-tight {lyric}",
                    "aria-live": "off",
                    if let Some(text) = line {
                        span { class: "line-clamp-2", "{text}" }
                    }
                }
            }
            div { class: "relative h-5",
                div {
                    class: "absolute inset-0 {seek}",
                    SeekRow { class: "-mt-1 text-xs" }
                }
                div {
                    class: "absolute inset-x-1 bottom-1 h-[3px] overflow-hidden rounded-full bg-surface-2 {thin}",
                    "aria-hidden": "true",
                    div { class: "h-full rounded-full bg-accent", style: "width:{pct}%" }
                }
            }
        }
    }
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("mini_tests.rs");
}
