//! The lyrics view used by the queue drawer and the fullscreen player.

use std::sync::Arc;

use dioxus::prelude::*;

use super::lyric_logic::{
    active_line, is_rtl, line_opacity, shifted, should_autoscroll, word_spans, WordSpan,
};
use super::lyrics_state::{self, lyrics_enabled, translation_for, LyricsView, LYRICS};
use super::lyrics_tools::LyricsTools;
use crate::features::tracks::state::library as library_state;
use crate::i18n::t;
use crate::model::{Lyrics, Track};
use crate::player::controller::{self as ctl, POSITION};
use crate::store::library;
use crate::ui::{cx, next_id, Button, ButtonVariant, Icon, Skeleton};

/// Lyrics for the playing song. `large` is the fullscreen look (giant type), `panel` the docked /
/// drawer look (big bold type); `on_dark` forces light text.
#[component]
pub fn LyricsPane(
    #[props(default)] large: bool,
    #[props(default)] panel: bool,
    #[props(default)] on_dark: bool,
) -> Element {
    let track = ctl::QUEUE.read().current().map(|e| e.track.clone());
    let enabled = lyrics_enabled();
    let wanted = track.clone();
    use_effect(use_reactive!(|enabled, wanted| {
        let _ = enabled;
        if let Some(t) = &wanted {
            lyrics_state::request(t, false);
        }
    }));
    let Some(track) = track else {
        return rsx! {
            Message { icon: "mic-vocal", title: t("player.no_track"), on_dark }
        };
    };
    let entry = LYRICS.read().clone();
    // The entry may still belong to the previous song for one render.
    let view = if entry.track_id.as_ref() == Some(&track.id) {
        entry.view
    } else {
        LyricsView::Loading
    };
    match view {
        LyricsView::Disabled | LyricsView::Idle if !enabled => {
            rsx! { OptIn { track, on_dark } }
        }
        LyricsView::Disabled | LyricsView::Idle | LyricsView::Loading => {
            rsx! { Loading {} }
        }
        LyricsView::NotFound => rsx! {
            Message { icon: "mic-vocal", title: t("lyrics.none"), on_dark }
        },
        LyricsView::Failed(msg) => rsx! {
            Failed { track, msg, on_dark }
        },
        LyricsView::Ready(lyrics) => {
            if lyrics.is_synced() {
                rsx! {
                    Synced { key: "{track.id}", track, lyrics, large, panel, on_dark }
                }
            } else {
                rsx! {
                    Plain { lyrics, large, panel, on_dark }
                }
            }
        }
    }
}

#[component]
fn Message(icon: String, title: String, on_dark: bool) -> Element {
    let tone = if on_dark { "text-fg/85" } else { "text-muted" };
    rsx! {
        div { class: "flex h-full min-h-48 flex-col items-center justify-center gap-4 px-6 text-center {tone}",
            span { class: "flex h-16 w-16 items-center justify-center rounded-full border border-line bg-hover text-accent-text",
                Icon { name: icon, size: 28 }
            }
            p { class: "ow-display max-w-[28ch] text-lg font-bold text-fg", "{title}" }
        }
    }
}

#[component]
fn Loading() -> Element {
    rsx! {
        div { class: "flex flex-col gap-4 p-6", role: "status",
            for w in ["w-3/4", "w-1/2", "w-2/3", "w-5/6", "w-1/3"] {
                Skeleton { key: "{w}", class: "h-5 {w}" }
            }
            p { class: "sr-only", {t("lyrics.loading")} }
        }
    }
}

#[component]
fn Failed(track: Track, msg: String, on_dark: bool) -> Element {
    rsx! {
        div { class: "flex h-full min-h-48 flex-col items-center justify-center gap-4 px-6 text-center",
            Message { icon: "circle-alert", title: msg, on_dark }
            Button {
                variant: ButtonVariant::Secondary,
                icon: "refresh-cw",
                                onclick: move |_| lyrics_state::request(&track, true),
                {t("common.retry")}
            }
        }
    }
}

/// Explains exactly what is sent to LRCLIB and asks for consent.
#[component]
fn OptIn(track: Track, on_dark: bool) -> Element {
    let tone = "text-fg";
    let muted = if on_dark { "text-fg/80" } else { "text-muted" };
    let card = if on_dark {
        "border-line-strong bg-bg/40"
    } else {
        "border-line bg-surface-2"
    };
    rsx! {
        div { class: "mx-auto flex max-w-[420px] flex-col gap-4 p-6",
            div { class: "flex flex-col gap-3 rounded-md border p-5 {card}",
                div { class: "flex items-center gap-2 {tone}",
                    Icon { name: "shield-check", size: 20 }
                    h3 { class: "text-base font-bold", {t("lyrics.disabled.title")} }
                }
                p { class: "text-sm {muted}", {t("lyrics.optin.sent")} }
                ul { class: "list-disc pl-5 text-sm {tone}",
                    li { {t("lyrics.optin.item_title")} }
                    li { {t("lyrics.optin.item_artist")} }
                    li { {t("lyrics.optin.item_album")} }
                    li { {t("lyrics.optin.item_length")} }
                }
                p { class: "text-sm {muted}", {t("lyrics.optin.nothing_else")} }
                Button {
                    icon: "check",
                    class: "self-start",
                    onclick: move |_| lyrics_state::enable(Some(&track)),
                    {t("lyrics.optin.enable")}
                }
            }
        }
    }
}

#[component]
fn Plain(lyrics: Arc<Lyrics>, large: bool, panel: bool, on_dark: bool) -> Element {
    let size = if large {
        "text-2xl font-bold"
    } else if panel {
        "text-xl font-bold"
    } else {
        "text-base"
    };
    let tone = "text-fg";
    let note = if on_dark { "text-fg/70" } else { "text-muted" };
    rsx! {
        div { class: "h-full overflow-y-auto px-6 py-4", "data-selectable": "true",
            p { class: "mb-3 flex items-center gap-2 text-xs {note}",
                Icon { name: "info", size: 14 }
                {t("lyrics.unsynced")}
            }
            p {
                class: "whitespace-pre-wrap leading-8 {size} {tone}",
                dir: is_rtl(&lyrics.plain).then_some("rtl"),
                "{lyrics.plain}"
            }
        }
    }
}

/// Karaoke fill of the active line, word by word (see `assets/lyrics_fill.js`).
pub(super) const LYRICS_FILL_JS: &str = include_str!("../../../assets/lyrics_fill.js");

fn now_ms() -> u64 {
    library::now_ms()
}

/// Smooth scrolling unless motion is reduced; keeps the active line near the middle.
fn scroll_js(box_id: &str, line_id: &str) -> String {
    format!(
        "(function(){{var c=document.getElementById('{box_id}'),e=document.getElementById('{line_id}');if(!c||!e)return;\
var rm=document.documentElement.hasAttribute('data-low-power')||(window.matchMedia&&window.matchMedia('(prefers-reduced-motion: reduce)').matches);\
var top=e.offsetTop-c.clientHeight/2+e.offsetHeight/2;c.scrollTo({{top:Math.max(0,top),behavior:rm?'auto':'smooth'}});}})();"
    )
}

#[component]
fn Synced(track: Track, lyrics: Arc<Lyrics>, large: bool, panel: bool, on_dark: bool) -> Element {
    let ids = use_hook(|| next_id("lyrics"));
    use_hook(|| {
        document::eval(LYRICS_FILL_JS);
    });
    let box_id = format!("{ids}-box");
    let mut last_manual = use_signal(|| None::<u64>);
    let lines = lyrics.clone();
    let offset_ms = library_state().lyric_offset(&track.id);
    let memo_id = track.id.clone();
    let active = use_memo(move || {
        let offset = library_state().lyric_offset(&memo_id);
        active_line(&lines.synced, shifted(*POSITION.read(), offset))
    });
    let (scroll_box, line_prefix) = (box_id.clone(), ids.clone());
    use_effect(move || {
        let Some(i) = active() else { return };
        if should_autoscroll(now_ms(), *last_manual.peek()) {
            document::eval(&scroll_js(&scroll_box, &format!("{line_prefix}-{i}")));
        }
    });
    let size = if large {
        "text-[calc(clamp(26px,3.3vw,52px)*var(--ow-lyric-scale))]"
    } else if panel {
        "text-[calc(26px*var(--ow-lyric-scale))]"
    } else {
        "text-[calc(14px*var(--ow-lyric-scale))]"
    };
    let translated = translation_for(&track.id, lyrics.synced.len());
    let translation_of = |i: usize| {
        translated
            .as_ref()
            .and_then(|t| t.get(i))
            .filter(|s| !s.is_empty())
            .cloned()
    };
    let mut manual = move || last_manual.set(Some(now_ms()));
    rsx! {
        div { class: "relative h-full",
            LyricsTools { track, lyrics: lyrics.clone(), on_dark }
            div {
                id: "{box_id}",
                class: "ow-fade-edges relative h-full overflow-y-auto px-6",
                "data-ow-offset": "{offset_ms}",
                tabindex: "0",
                role: "region",
                "aria-label": t("lyrics.title"),
                onwheel: move |_| manual(),
                ontouchmove: move |_| manual(),
                onkeydown: move |e| {
                    if matches!(
                        e.key(),
                        Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown | Key::Home | Key::End
                    ) {
                        manual();
                    }
                },
                // Spacers let the first and last line reach the middle of the box.
                div { class: "h-[45%] flex-none", "aria-hidden": "true" }
                for (i , line) in lyrics.synced.iter().enumerate() {
                    LyricLineRow {
                        key: "{i}",
                        id: format!("{ids}-{i}"),
                        text: line.text.clone(),
                        words: word_spans(&lyrics.synced, i),
                        translation: translation_of(i),
                        start_ms: line.start_ms,
                        active: active() == Some(i),
                        size: size.to_string(),
                        opacity: line_opacity(active(), i),
                        on_dark,
                    }
                }
                div { class: "h-[45%] flex-none", "aria-hidden": "true" }
            }
        }
    }
}

#[component]
fn LyricLineRow(
    id: String,
    text: String,
    words: Vec<WordSpan>,
    translation: Option<String>,
    start_ms: u64,
    active: bool,
    size: String,
    opacity: f32,
    on_dark: bool,
) -> Element {
    if text.trim().is_empty() {
        return rsx! {
            div { id: "{id}", class: "h-6", "aria-hidden": "true" }
        };
    }
    let rtl = is_rtl(&text);
    let class = cx(&[
        "ow-lyric ow-display block w-full rounded-sm py-2 font-extrabold leading-[1.15] tracking-tight text-fg",
        if rtl { "text-right" } else { "text-left" },
        &size,
    ]);
    let tr_rtl = translation.as_deref().is_some_and(is_rtl);
    rsx! {
        button {
            id: "{id}",
            r#type: "button",
            class: "{class}",
            dir: rtl.then_some("rtl"),
            style: "--lo:{opacity}",
            "data-dark": on_dark.then_some("true"),
            "aria-current": active.then_some("true"),
            onclick: move |_| ctl::seek_to(start_ms as f64 / 1000.0),
            for w in words {
                span {
                    class: "ow-w",
                    "data-s": "{w.start_ms}",
                    "data-e": "{w.end_ms}",
                    "{w.text}"
                }
            }
            if let Some(tr) = translation {
                span {
                    class: "mt-1 block text-sm font-semibold leading-tight opacity-80",
                    dir: if tr_rtl { "rtl" } else { "auto" },
                    "{tr}"
                }
            }
        }
    }
}
