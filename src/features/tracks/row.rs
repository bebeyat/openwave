//! One 56px row of a `TrackList`: index / play, cover, title + artist, album, duration, like, menu.

use dioxus::prelude::*;

use super::actions::{open_album, open_artist};
use super::menu::{MenuExtras, TrackMenuItems};
use super::state::toggle_like;
use crate::i18n::{t, t_args};
use crate::model::Track;
use crate::ui::{ContextMenu, Cover, DropdownMenu, Icon, IconButton, IconButtonSize, MenuAlign};
use crate::util::format_time;

/// Grid template shared by the rows and the sticky header of a track table.
pub fn columns(show_album: bool, show_extra: bool) -> &'static str {
    match (show_album, show_extra) {
        (false, false) => "grid-cols-[40px_minmax(0,1fr)_56px_40px_40px]",
        (true, false) => "grid-cols-[40px_minmax(0,2fr)_minmax(0,1.3fr)_56px_40px_40px]",
        (false, true) => "grid-cols-[40px_minmax(0,1fr)_72px_56px_40px_40px]",
        (true, true) => "grid-cols-[40px_minmax(0,2fr)_minmax(0,1.3fr)_72px_56px_40px_40px]",
    }
}

/// Equalizer glyph of the playing row (three bars; still while paused).
#[component]
pub fn Equalizer(#[props(default)] paused: bool) -> Element {
    rsx! {
        span { class: "ow-eq", "data-paused": paused.then_some("true"), "aria-hidden": "true",
            i {}
            i {}
            i {}
        }
    }
}

/// "3:45", or an empty string when the duration is unknown.
pub fn duration_label(ms: u64) -> String {
    if ms == 0 {
        String::new()
    } else {
        format_time(ms as f64 / 1000.0)
    }
}

#[component]
fn ArtistLinks(track: Track) -> Element {
    if track.artists.is_empty() {
        let key = if track.is_local() {
            "common.local_file"
        } else {
            "common.unknown_artist"
        };
        return rsx! { "{t(key)}" };
    }
    let last = track.artists.len() - 1;
    rsx! {
        for (i, a) in track.artists.iter().enumerate() {
            {
                let id = a.id.to_string();
                let sep = if i < last { ", " } else { "" };
                rsx! {
                    if id.is_empty() {
                        span { "{a.name}{sep}" }
                    } else {
                        button {
                            r#type: "button",
                            class: "rounded-sm hover:text-fg hover:underline",
                            onclick: move |_| open_artist(&id),
                            "{a.name}"
                        }
                        "{sep}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn TrackRow(
    track: Track,
    index: usize,
    count: usize,
    liked: bool,
    current: bool,
    playing: bool,
    #[props(default = true)] show_album: bool,
    /// Text for the optional extra column (e.g. the time a song was played).
    #[props(default)]
    extra: Option<String>,
    #[props(default)] reorderable: bool,
    #[props(default)] removable: bool,
    /// Saved for offline listening: a small mark after the title.
    #[props(default)]
    downloaded: bool,
    /// Bulk selection mode: a checkbox replaces the number and a click toggles the row.
    #[props(default)]
    selecting: bool,
    #[props(default)] selected: bool,
    #[props(default)] on_select: EventHandler<usize>,
    #[props(default)] dragging: bool,
    #[props(default)] drop_target: bool,
    #[props(default)] drop_below: bool,
    on_play: EventHandler<usize>,
    on_double: EventHandler<usize>,
    on_add_to_playlist: EventHandler<Track>,
    on_remove: EventHandler<usize>,
    on_move: EventHandler<(usize, isize)>,
    on_drag_start: EventHandler<usize>,
    on_drag_over: EventHandler<usize>,
) -> Element {
    let extras = MenuExtras {
        can_move_up: reorderable && index > 0,
        can_move_down: reorderable && index + 1 < count,
        removable,
    };
    let show_extra = extra.is_some();
    let cols = columns(show_album, show_extra);
    let tone = "";
    let mark = match (drop_target, drop_below) {
        (true, true) => "shadow-[inset_0_-2px_0_var(--ow-accent)]",
        (true, false) => "shadow-[inset_0_2px_0_var(--ow-accent)]",
        _ => "",
    };
    let pad = if reorderable { "pl-6 pr-2" } else { "px-2" };
    let faded = if dragging { "opacity-40" } else { "" };
    let title_tone = if current {
        "text-accent-text"
    } else {
        "text-fg"
    };
    let pause_now = current && playing;
    let play_label = if pause_now {
        t("common.pause")
    } else {
        t_args("home.play_card", &[("title", track.title.as_str())])
    };
    let hover_icon = if pause_now { "pause" } else { "play" };
    let like_label = if liked {
        t("track.unlike")
    } else {
        t("track.like")
    };
    let menu_label = t("common.more");
    let album = track.album.clone();
    let album_title = album.as_ref().map(|a| a.title.to_string());
    let like_track = track.clone();
    let cover = track.cover.as_ref().map(|c| c.url.to_string());
    let duration = duration_label(track.duration_ms);
    let items = move || {
        rsx! {
            TrackMenuItems {
                track: track.clone(),
                liked,
                extras,
                on_add_to_playlist,
                on_move: move |d| on_move.call((index, d)),
                on_remove: move |_| on_remove.call(index),
            }
        }
    };
    let (title, artist_track) = (like_track.title.clone(), like_track.clone());
    let artist_names = like_track.artists_label();
    rsx! {
        ContextMenu { class: "block", label: menu_label.clone(), menu: items(),
            div {
                role: "listitem",
                "data-index": "{index}",
                "aria-current": current.then_some("true"),
                class: "ow-trow group relative grid h-14 items-center gap-3 rounded-lg {pad} {cols} {tone} {mark} {faded}",
                onclick: move |_| {
                    if selecting {
                        on_select.call(index);
                    }
                },
                ondoubleclick: move |_| {
                    if !selecting {
                        on_double.call(index);
                    }
                },
                onmouseenter: move |_| on_drag_over.call(index),
                onkeydown: move |e| {
                    if reorderable && e.modifiers().alt() {
                        match e.key() {
                            Key::ArrowUp => {
                                e.prevent_default();
                                on_move.call((index, -1));
                            }
                            Key::ArrowDown => {
                                e.prevent_default();
                                on_move.call((index, 1));
                            }
                            _ => {}
                        }
                    }
                },
                if reorderable {
                    div {
                        "aria-hidden": "true",
                        title: t("playlist.drag_hint"),
                        class: "absolute -left-0.5 top-1/2 flex h-8 w-4 -translate-y-1/2 cursor-grab items-center justify-center text-muted opacity-0 hover:text-fg group-focus-within:opacity-100 group-hover:opacity-100",
                        onmousedown: move |e| {
                            e.prevent_default();
                            on_drag_start.call(index);
                        },
                        Icon { name: "grip-vertical", size: 16 }
                    }
                }
                if selecting {
                    button {
                        r#type: "button",
                        role: "checkbox",
                        "aria-checked": "{selected}",
                        "aria-label": "{title}",
                        class: "flex h-10 w-10 items-center justify-center rounded-full",
                        onclick: move |e| {
                            e.stop_propagation();
                            on_select.call(index);
                        },
                        Icon {
                            name: if selected { "checkbox-checked" } else { "checkbox" },
                            size: 22,
                            class: "text-muted",
                        }
                    }
                } else {
                button {
                    r#type: "button",
                    "aria-label": "{play_label}",
                    class: "flex h-10 w-10 items-center justify-center rounded-full text-sm tabular-nums text-muted hover:text-fg",
                    onclick: move |_| on_play.call(index),
                    span { class: "group-focus-within:hidden group-hover:hidden",
                        if current {
                            span { class: "text-accent-text", Equalizer { paused: !playing } }
                        } else {
                            span { class: if current { "font-semibold text-accent-text" } else { "" }, "{index + 1}" }
                        }
                    }
                    span { class: "hidden text-fg group-focus-within:inline-flex group-hover:inline-flex",
                        Icon { name: hover_icon, size: 18, filled: true }
                    }
                }
                }
                div { class: "flex min-w-0 items-center gap-3",
                    Cover { src: cover, size: 40, alt: "" }
                    div { class: "min-w-0",
                        p { class: "flex min-w-0 items-center gap-1.5 text-sm font-semibold {title_tone}", title: "{title}",
                            span { class: "truncate", "{title}" }
                            if downloaded {
                                span { class: "flex-none text-accent-text", title: t("downloads.mark"),
                                    Icon { name: "download", size: 13 }
                                }
                            }
                        }
                        p { class: "truncate text-sm text-muted", title: "{artist_names}",
                            ArtistLinks { track: artist_track }
                        }
                    }
                }
                if show_album {
                    div {
                        class: "min-w-0 truncate text-sm text-muted",
                        title: album_title,
                        if let Some(a) = album {
                            if a.id.is_empty() {
                                "{a.title}"
                            } else {
                                button {
                                    r#type: "button",
                                    class: "max-w-full truncate rounded-sm hover:text-fg hover:underline",
                                    onclick: move |_| open_album(&a.id),
                                    "{a.title}"
                                }
                            }
                        }
                    }
                }
                if let Some(x) = extra {
                    span { class: "text-sm tabular-nums text-muted", "{x}" }
                }
                span { class: "text-right text-sm tabular-nums text-muted", "{duration}" }
                IconButton {
                    icon: "heart",
                    label: like_label,
                    size: IconButtonSize::Sm,
                    pressed: Some(liked),
                    filled: liked,
                    onclick: move |_| toggle_like(&like_track),
                }
                DropdownMenu {
                    icon: "ellipsis",
                    aria_label: menu_label,
                    align: MenuAlign::End,
                    {items()}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::duration_label;

    #[test]
    fn duration_label_formats_or_hides() {
        assert_eq!(duration_label(0), "");
        assert_eq!(duration_label(225_000), "3:45");
        assert_eq!(duration_label(3_725_000), "1:02:05");
    }
}
