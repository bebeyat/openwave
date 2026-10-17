//! Controls shared by the player bar and the fullscreen player: transport buttons, the seek row
//! and the volume control. Each reads only the signals it needs so a position tick re-renders
//! the seek row alone.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::player::controller as ctl;
use crate::player::queue::Repeat;
use crate::ui::{GradientPlayButton, IconButton, IconButtonSize, PlaySize, Slider, Tooltip};
use crate::util::format_time;

/// Shuffle, previous, play/pause, next, repeat.
#[component]
pub fn Transport(
    #[props(default)] big: bool,
    /// Show shuffle and repeat (Özelleştirme can hide them in the player bar).
    #[props(default = true)]
    modes: bool,
) -> Element {
    let queue = ctl::QUEUE.read();
    let (shuffle, repeat) = (queue.shuffle(), queue.repeat());
    let has_prev_or_next = !queue.is_empty();
    drop(queue);
    let status = *ctl::STATUS.read();
    let active = status.is_active();
    let play_label = if active {
        t("player.pause")
    } else {
        t("player.play")
    };
    let play_size = if big { "h-16! w-16!" } else { "h-10! w-10!" };
    let (repeat_icon, repeat_label) = match repeat {
        Repeat::Off => ("repeat", t("player.repeat.off")),
        Repeat::All => ("repeat", t("player.repeat.all")),
        Repeat::One => ("repeat-1", t("player.repeat.one")),
    };
    rsx! {
        div { class: "flex items-center justify-center gap-3", role: "group", "aria-label": t("player.region"),
            if modes {
            IconButton {
                icon: "shuffle",
                label: t("player.shuffle"),
                size: IconButtonSize::Sm,
                pressed: Some(shuffle),
                indicator: shuffle,
                onclick: move |_| ctl::toggle_shuffle(),
            }
            }
            IconButton {
                icon: "skip-back",
                label: t("player.previous"),
                shortcut: "Ctrl+←",
                size: IconButtonSize::Sm,
                disabled: !has_prev_or_next,
                onclick: move |_| ctl::previous(),
            }
            Tooltip { label: play_label.clone(), shortcut: "Space",
                GradientPlayButton {
                    playing: active,
                    label: play_label,
                    size: PlaySize::Md,
                    disabled: !has_prev_or_next,
                    class: "ow-play-pulse {play_size}",
                    onclick: move |_| ctl::toggle_pause(),
                }
            }
            IconButton {
                icon: "skip-forward",
                label: t("player.next"),
                shortcut: "Ctrl+→",
                size: IconButtonSize::Sm,
                disabled: !has_prev_or_next,
                onclick: move |_| ctl::next(),
            }
            if modes {
            IconButton {
                icon: repeat_icon,
                label: repeat_label,
                size: IconButtonSize::Sm,
                pressed: Some(repeat != Repeat::Off),
                indicator: repeat != Repeat::Off,
                onclick: move |_| ctl::cycle_repeat(),
            }
            }
        }
    }
}

/// Elapsed time, seek slider, total time.
#[component]
pub fn SeekRow(#[props(default)] class: String) -> Element {
    let pos = *ctl::POSITION.read();
    let _ = ctl::DURATION.read();
    let dur = ctl::display_duration();
    let max = dur.max(1.0);
    let elapsed = format_time(pos);
    let total = format_time(dur);
    let shown = pos.min(max);
    let text = format!("{elapsed} / {total}");
    rsx! {
        div { class: "flex w-full items-center gap-3 {class}",
            span { class: "w-10 flex-none text-right text-xs tabular-nums text-muted", "{elapsed}" }
            Slider {
                value: shown,
                max,
                step: 1.0,
                big_step: 10.0,
                label: t("player.seek"),
                value_text: text,
                disabled: dur <= 0.0,
                class: "h-7!",
                on_commit: ctl::seek_to,
            }
            span { class: "w-10 flex-none text-xs tabular-nums text-muted", "{total}" }
        }
    }
}

/// Mute button and volume slider.
#[component]
pub fn VolumeControl(#[props(default)] slider_class: String) -> Element {
    let volume = *ctl::VOLUME.read();
    let muted = *ctl::MUTED.read();
    let level = if muted { 0.0 } else { volume };
    let icon = if muted || volume <= 0.0 {
        "volume-x"
    } else if volume < 0.5 {
        "volume-1"
    } else {
        "volume-2"
    };
    let label = if muted {
        t("player.unmute")
    } else {
        t("player.mute")
    };
    let percent = (level * 100.0).round();
    let text = format!("{} {percent}%", t("player.volume"));
    rsx! {
        div { class: "flex items-center gap-1",
            IconButton {
                icon,
                label,
                shortcut: "M",
                size: IconButtonSize::Sm,
                pressed: Some(muted),
                onclick: move |_| ctl::toggle_mute(),
            }
            div { class: "flex-none {slider_class}",
                Slider {
                    value: percent,
                    max: 100.0,
                    step: 1.0,
                    big_step: 10.0,
                    label: t("player.volume"),
                    value_text: text,
                    on_input: move |v| ctl::set_volume(v / 100.0),
                }
            }
        }
    }
}
