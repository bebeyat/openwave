//! Oynatma and Sözler sections.

use std::sync::Arc;

use dioxus::prelude::*;

use super::effects::EffectsCard;
use super::rows::{Card, Row, SectionTitle, ToggleRow};
use crate::i18n::t;
use crate::store::{AudioQuality, DoubleClickAction, Settings, SettingsChange};
use crate::ui::{Banner, BannerKind, SegmentedControl, TabItem};

pub fn double_click_value(a: DoubleClickAction) -> &'static str {
    match a {
        DoubleClickAction::PlayNow => "play_now",
        DoubleClickAction::AddToQueue => "add_to_queue",
    }
}

pub fn parse_double_click(v: &str) -> DoubleClickAction {
    match v {
        "add_to_queue" => DoubleClickAction::AddToQueue,
        _ => DoubleClickAction::PlayNow,
    }
}

pub fn quality_value(q: AudioQuality) -> &'static str {
    match q {
        AudioQuality::Economy => "economy",
        AudioQuality::High => "high",
    }
}

pub fn parse_quality(v: &str) -> AudioQuality {
    match v {
        "economy" => AudioQuality::Economy,
        _ => AudioQuality::High,
    }
}

#[component]
pub fn Playback(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    let clicks = vec![
        TabItem::new("play_now", t("settings.playback.double_click.play_now")),
        TabItem::new(
            "add_to_queue",
            t("settings.playback.double_click.add_to_queue"),
        ),
    ];
    let qualities = vec![
        TabItem::new("economy", t("settings.playback.quality.economy.short")),
        TabItem::new("high", t("settings.playback.quality.high")),
    ];
    rsx! {
        SectionTitle { title: t("settings.section.playback"), icon: "play" }
        Card {
            ToggleRow {
                label: t("settings.playback.autoplay"),
                help: t("settings.playback.autoplay.help"),
                checked: settings.autoplay_similar,
                on_change: move |v| on_change.call(SettingsChange::AutoplaySimilar(v)),
            }
            Row { label: t("settings.playback.double_click"), help: t("settings.playback.double_click.help"),
                SegmentedControl {
                    items: clicks,
                    value: double_click_value(settings.double_click),
                    group: "dblclick",
                    class: "ow-seg",
                    aria_label: t("settings.playback.double_click"),
                    on_change: move |v: String| on_change.call(SettingsChange::DoubleClick(parse_double_click(&v))),
                }
            }
            ToggleRow {
                label: t("settings.playback.resume"),
                help: t("settings.playback.resume.help"),
                checked: settings.resume_queue,
                on_change: move |v| on_change.call(SettingsChange::ResumeQueue(v)),
            }
            ToggleRow {
                label: t("settings.playback.pause_on_device_change"),
                help: t("settings.playback.pause_on_device_change.help"),
                checked: settings.pause_on_device_change,
                on_change: move |v| on_change.call(SettingsChange::PauseOnDeviceChange(v)),
            }
            Row { label: t("settings.playback.quality"), help: t("settings.playback.quality.help"),
                SegmentedControl {
                    items: qualities,
                    value: quality_value(settings.audio_quality),
                    group: "quality",
                    class: "ow-seg",
                    aria_label: t("settings.playback.quality"),
                    on_change: move |v: String| on_change.call(SettingsChange::AudioQuality(parse_quality(&v))),
                }
            }
        }
        EffectsCard { settings: settings.clone(), on_change }
    }
}

#[component]
pub fn Lyrics(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    rsx! {
        SectionTitle { title: t("settings.section.lyrics"), intro: t("settings.lyrics.intro"), icon: "mic-vocal" }
        Card {
            ToggleRow {
                label: t("settings.lyrics.optin"),
                help: t("settings.lyrics.optin.help"),
                checked: settings.lyrics_enabled,
                on_change: move |v| on_change.call(SettingsChange::LyricsEnabled(v)),
            }
        }
        Banner { class: "mt-4", kind: BannerKind::Info, title: t("settings.lyrics.what_title"),
            p { {t("settings.lyrics.explain")} }
            p { class: "mt-2", {t("settings.lyrics.sent")} }
            p { class: "mt-1", {t("settings.lyrics.not_sent")} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_values_round_trip() {
        for a in [DoubleClickAction::PlayNow, DoubleClickAction::AddToQueue] {
            assert_eq!(parse_double_click(double_click_value(a)), a);
        }
        for q in [AudioQuality::Economy, AudioQuality::High] {
            assert_eq!(parse_quality(quality_value(q)), q);
        }
    }

    #[test]
    fn lyrics_change_is_off_by_default_and_toggles() {
        let s = Settings::default();
        assert!(!s.lyrics_enabled);
        assert!(s.apply(SettingsChange::LyricsEnabled(true)).lyrics_enabled);
    }
}
