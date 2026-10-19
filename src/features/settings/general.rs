//! Genel: language, theme, accent colour, low-power mode. Everything applies instantly.

use std::sync::Arc;

use dioxus::prelude::*;

use super::integrations::DiscordCard;
use super::rows::{Card, Row, SectionTitle, ToggleRow};
use crate::i18n::{t, Lang};
use crate::store::{LanguagePref, Settings, SettingsChange, ThemePref, ACCENT_IDS};
use crate::ui::tabs::roving_target;
use crate::ui::{Icon, SegmentedControl, Select, SelectOption, TabItem};

pub fn language_value(p: LanguagePref) -> &'static str {
    p.lang().map_or("system", Lang::code)
}

pub fn parse_language(v: &str) -> LanguagePref {
    Lang::from_code(v).map_or(LanguagePref::System, LanguagePref::from_lang)
}

/// "Sistem" first, then every language under its own name.
fn language_options() -> Vec<SelectOption> {
    std::iter::once(SelectOption {
        value: "system".into(),
        label: t("settings.general.language.system"),
    })
    .chain(Lang::ALL.into_iter().map(|l| SelectOption {
        value: l.code().into(),
        label: l.native_name().into(),
    }))
    .collect()
}

pub fn theme_value(p: ThemePref) -> &'static str {
    match p {
        ThemePref::Light => "light",
        ThemePref::Dark => "dark",
        ThemePref::System => "system",
    }
}

pub fn parse_theme(v: &str) -> ThemePref {
    match v {
        "light" => ThemePref::Light,
        "dark" => ThemePref::Dark,
        _ => ThemePref::System,
    }
}

/// Preset colour pairs (gradient stops, mirroring the `data-accent` presets in the CSS). The real
/// accent of the app still comes from CSS; these only paint the chips and the live preview.
fn swatch(id: &str) -> (&'static str, &'static str) {
    match id {
        "blue" => ("#5aa2ff", "#a08bff"),
        "violet" => ("#a78bfa", "#f28ad8"),
        "pink" => ("#fb7185", "#ffa15c"),
        "orange" => ("#fb923c", "#ffd25a"),
        "green" => ("#4ade80", "#2dd4bf"),
        _ => ("#2dd4bf", "#8a7cff"),
    }
}

/// What the selected preset looks like on the things that carry the accent: the round play
/// button, the seek fill and a pill. Painted from the preset itself, so it updates at once.
#[component]
fn AccentPreview(id: String) -> Element {
    let (a, b) = swatch(&id);
    let name = t(&format!("settings.general.accent.{id}"));
    rsx! {
        div {
            class: "flex items-center gap-4 rounded-lg border border-line bg-surface px-4 py-3",
            style: "--pa:{a};--pb:{b}",
            "aria-hidden": "true",
            span { class: "ow-prev-fill ow-prev-glow inline-flex h-10 w-10 flex-none items-center justify-center rounded-full",
                Icon { name: "play", size: 18 }
            }
            div { class: "relative h-1.5 min-w-0 flex-1 rounded-full bg-press",
                div { class: "ow-prev-bar h-full w-3/5 rounded-full" }
                span { class: "absolute left-3/5 top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg" }
            }
            span { class: "ow-prev-fill flex-none rounded-full px-3 py-1 text-xs font-bold", "{name}" }
        }
    }
}

#[component]
pub(super) fn AccentPicker(current: String, on_change: EventHandler<String>) -> Element {
    let ids: Vec<String> = ACCENT_IDS.iter().map(|s| s.to_string()).collect();
    let at = ids.iter().position(|i| *i == current).unwrap_or(0);
    let group = ids.clone();
    rsx! {
        div { class: "flex flex-col gap-3",
            div {
                role: "radiogroup",
                "aria-label": t("settings.general.accent"),
                class: "flex flex-wrap items-center gap-3",
                onkeydown: move |e| {
                    if let Some(n) = roving_target(&e.key(), at, group.len()) {
                        e.prevent_default();
                        on_change.call(group[n].clone());
                        document::eval(&format!("window.ow.focusId('ow-accent-{}')", group[n]));
                    }
                },
                for id in ids {
                    {
                        let selected = id == current;
                        let name = t(&format!("settings.general.accent.{id}"));
                        let pick = id.clone();
                        let (a, b) = swatch(&id);
                        let ring = if selected {
                            "outline-2 outline-offset-[3px] outline-fg scale-105"
                        } else {
                            "hover:scale-110"
                        };
                        rsx! {
                            button {
                                key: "{id}",
                                id: "ow-accent-{id}",
                                r#type: "button",
                                role: "radio",
                                title: "{name}",
                                "aria-label": "{name}",
                                "aria-checked": "{selected}",
                                tabindex: if selected { "0" } else { "-1" },
                                class: "ow-prev-glow flex h-10 w-10 items-center justify-center rounded-full text-[#0c0a12] transition-transform duration-(--ow-dur-hover) {ring}",
                                style: "--pa:{a};--pb:{b};background:{a}",
                                onclick: move |_| on_change.call(pick.clone()),
                                if selected {
                                    Icon { name: "check", size: 18 }
                                }
                            }
                        }
                    }
                }
            }
            AccentPreview { id: current.clone() }
        }
    }
}

#[component]
pub fn General(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    let themes = vec![
        TabItem::new("light", t("settings.general.theme.light")).icon("sun"),
        TabItem::new("dark", t("settings.general.theme.dark")).icon("moon"),
        TabItem::new("system", t("settings.general.theme.system")).icon("monitor"),
    ];
    rsx! {
        SectionTitle { title: t("settings.section.general"), icon: "sliders-horizontal" }
        Card {
            Row { label: t("settings.general.language"), help: t("settings.general.language.help"),
                Select {
                    options: language_options(),
                    value: language_value(settings.language),
                    aria_label: t("settings.general.language"),
                    on_change: move |v: String| on_change.call(SettingsChange::Language(parse_language(&v))),
                }
            }
            Row { label: t("settings.general.theme"), help: t("settings.general.theme.help"),
                SegmentedControl {
                    items: themes,
                    value: theme_value(settings.theme),
                    group: "theme",
                    class: "ow-seg",
                    aria_label: t("settings.general.theme"),
                    on_change: move |v: String| on_change.call(SettingsChange::Theme(parse_theme(&v))),
                }
            }
            Row { label: t("settings.general.accent"), help: t("settings.general.accent.help"), stack: true,
                AccentPicker {
                    current: settings.accent.clone(),
                    on_change: move |id: String| on_change.call(SettingsChange::Accent(id)),
                }
            }
            ToggleRow {
                label: t("settings.general.low_power"),
                help: t("settings.general.low_power.help"),
                checked: settings.low_power,
                on_change: move |v| on_change.call(SettingsChange::LowPower(v)),
            }
        }
        DiscordCard { settings: settings.clone(), on_change }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_values_round_trip() {
        assert_eq!(
            parse_language(language_value(LanguagePref::System)),
            LanguagePref::System
        );
        for lang in Lang::ALL {
            let p = LanguagePref::from_lang(lang);
            assert_eq!(parse_language(language_value(p)), p);
            assert_eq!(p.lang(), Some(lang));
        }
        assert_eq!(language_options().len(), Lang::ALL.len() + 1);
        for p in [ThemePref::Light, ThemePref::Dark, ThemePref::System] {
            assert_eq!(parse_theme(theme_value(p)), p);
        }
        assert_eq!(parse_theme("neon"), ThemePref::System);
    }

    #[test]
    fn every_accent_has_a_swatch() {
        for id in ACCENT_IDS {
            let (a, b) = swatch(id);
            assert!(a.starts_with('#') && b.starts_with('#') && a != b, "{id}");
        }
        assert_ne!(swatch("pink"), swatch("teal"));
    }
}
