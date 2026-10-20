//! Mini player: the whole (small, always-on-top) window shows the cover, the title, the transport
//! and the seek bar. The rest of the app stays mounted underneath, so playback is untouched.
//!
//! When the pointer has been away for [`IDLE_AFTER`] and the song has synced lyrics, the transport
//! fades out for the lyric lines and the seek bar thins to a progress line; the pointer
//! coming back reverses it.

use std::time::Duration;

use dioxus::desktop::use_window;
use dioxus::prelude::*;

use super::controls::{SeekRow, Transport};
use super::lyric_logic::{active_line, shifted, word_spans};
use super::lyrics::LYRICS_FILL_JS;
use super::lyrics_state::{self, LyricsView, LYRICS};
use crate::features::shell::titlebar::DragRegion;
use crate::features::shell::window::{request_close, toggle_mini, MINI};
use crate::features::tracks::state::library;
use crate::i18n::t;
use std::sync::Arc;

use crate::model::Lyrics;
use crate::player::controller::{self as ctl, POSITION};
use crate::ui::{CoverThumb, IconButton, IconButtonSize, TooltipAlign, TooltipSide};

const IDLE_AFTER: Duration = Duration::from_secs(3);
/// Shared by both layers of each swap; motion-reduce turns the animation off.
const FADE: &str = "transition-all duration-300 ease-out motion-reduce:transition-none";

/// Height of one lyric row; the stack moves by whole rows.
const ROW_PX: usize = 48;

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
    use_hook(leave_later);
    // The karaoke fill and glow of the lyrics views (idempotent: it replaces an earlier copy).
    use_hook(|| {
        document::eval(LYRICS_FILL_JS);
    });

    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    let track_id = track.as_ref().map(|t| t.id.clone());
    let request_track = track.clone();
    use_effect(use_reactive!(|track_id| {
        let _ = track_id;
        if let Some(t) = &request_track {
            lyrics_state::request(t, false);
        }
    }));
    let offset_ms = track.as_ref().map_or(0, |t| library().lyric_offset(&t.id));
    let synced = match (&LYRICS.read().view, &track) {
        (LyricsView::Ready(l), Some(t))
            if l.is_synced() && LYRICS.read().track_id.as_ref() == Some(&t.id) =>
        {
            Some(l.clone())
        }
        _ => None,
    };
    let active = synced
        .as_ref()
        .and_then(|l| active_line(&l.synced, shifted(*POSITION.read(), offset_ms)));
    let quiet = idle() && synced.is_some();

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
                div {
                    class: "absolute inset-0 overflow-hidden {lyric}",
                    if let Some(lyrics) = synced {
                        MiniLyrics { lyrics, active, offset_ms }
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

/// Every line stays mounted and only `aria-current` moves, exactly like the big lyrics views, so
/// `lyrics_fill.js` and the `.ow-lyric` styles fill, glow and dim the lines the same way; the
/// stack slides by one row per line.
#[component]
fn MiniLyrics(lyrics: Arc<Lyrics>, active: Option<usize>, offset_ms: i32) -> Element {
    let shift = active.unwrap_or(0) * ROW_PX;
    rsx! {
        div {
            class: "transition-transform duration-500 ease-out motion-reduce:transition-none",
            style: "transform:translateY(-{shift}px)",
            "data-ow-offset": "{offset_ms}",
            for (i, line) in lyrics.synced.iter().enumerate() {
                if line.text.trim().is_empty() {
                    p { key: "{i}", class: "flex h-12 items-center justify-center text-lg text-muted", "♪" }
                } else {
                    p {
                        key: "{i}",
                        class: "ow-lyric ow-display flex h-12 items-center justify-center px-2 text-center text-[20px] font-extrabold leading-[1.15] tracking-tight text-fg",
                        style: if active == Some(i) { "--lo:1" } else { "--lo:.35" },
                        "data-dark": "true",
                        "aria-current": (active == Some(i)).then_some("true"),
                        span { class: "line-clamp-2",
                            for w in word_spans(&lyrics.synced, i) {
                                span { class: "ow-w", "data-s": "{w.start_ms}", "data-e": "{w.end_ms}", "{w.text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
