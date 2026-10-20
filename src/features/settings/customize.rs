//! Özelleştirme: colours, backdrop, glass, sizes, motion, Home sections, sidebar, player bar,
//! fullscreen artwork and lyrics. Sliders preview live while dragged (`boot::CUSTOM`) and are
//! saved on release; switches and choices are saved at once.

use dioxus::prelude::*;

use super::boot::CUSTOM;
use super::general::AccentPicker;
use super::rows::{Card, Row, SectionTitle, ToggleRow};
use crate::i18n::t;
use crate::store::customize::{
    parse_hex, AccentMode, Customization, FontChoice, LyricsAlign, SizeChoice, BLUR_MAX,
    BRIGHTNESS_RANGE, DIM_MAX, GAP_MAX, LYRICS_SIZE_RANGE, PANEL_OPACITY_RANGE, RADIUS_MAX,
    UI_SCALE_RANGE,
};
use crate::store::{Settings, SettingsChange};
use crate::ui::{Button, ButtonVariant, SegmentedControl, Slider, TabItem};

fn accent_value(m: AccentMode) -> &'static str {
    match m {
        AccentMode::Cover => "cover",
        AccentMode::Preset => "preset",
        AccentMode::Custom => "custom",
    }
}

fn parse_accent(v: &str) -> AccentMode {
    match v {
        "preset" => AccentMode::Preset,
        "custom" => AccentMode::Custom,
        _ => AccentMode::Cover,
    }
}

fn size_value(s: SizeChoice) -> &'static str {
    match s {
        SizeChoice::Small => "s",
        SizeChoice::Medium => "m",
        SizeChoice::Large => "l",
        SizeChoice::XLarge => "xl",
    }
}

fn parse_size(v: &str) -> SizeChoice {
    match v {
        "s" => SizeChoice::Small,
        "l" => SizeChoice::Large,
        "xl" => SizeChoice::XLarge,
        _ => SizeChoice::Medium,
    }
}

fn font_value(f: FontChoice) -> &'static str {
    match f {
        FontChoice::Inter => "inter",
        FontChoice::Jakarta => "jakarta",
        FontChoice::System => "system",
    }
}

fn parse_font(v: &str) -> FontChoice {
    match v {
        "jakarta" => FontChoice::Jakarta,
        "system" => FontChoice::System,
        _ => FontChoice::Inter,
    }
}

fn size_items() -> Vec<TabItem> {
    vec![
        TabItem::new("s", t("custom.size.s")),
        TabItem::new("m", t("custom.size.m")),
        TabItem::new("l", t("custom.size.l")),
        TabItem::new("xl", t("custom.size.xl")),
    ]
}

/// A labelled slider over a `u8` setting: live preview while dragging, saved on release.
#[component]
fn RangeRow(
    #[props(into)] label: String,
    #[props(into, default)] help: Option<String>,
    value: u8,
    min: u8,
    max: u8,
    #[props(default = 1)] step: u8,
    #[props(into)] unit: String,
    on_preview: EventHandler<u8>,
    on_commit: EventHandler<u8>,
) -> Element {
    let readout = format!("{value}{unit}");
    let text = format!("{label} {readout}");
    rsx! {
        Row { label: label.clone(), help,
            div { class: "flex w-[260px] items-center gap-3 max-[1099px]:w-[200px]",
                Slider {
                    value: f64::from(value),
                    min: f64::from(min),
                    max: f64::from(max),
                    step: f64::from(step),
                    big_step: f64::from(step) * 5.0,
                    label,
                    value_text: text,
                    class: "h-7!",
                    on_input: move |v: f64| on_preview.call(v.round().clamp(0.0, 255.0) as u8),
                    on_commit: move |v: f64| on_commit.call(v.round().clamp(0.0, 255.0) as u8),
                }
                span { class: "w-14 flex-none text-right text-sm font-semibold tabular-nums text-fg", "{readout}" }
            }
        }
    }
}

/// `#rrggbb` picker: the native colour well plus the hex text.
#[component]
fn ColorRow(value: String, on_change: EventHandler<String>) -> Element {
    let mut draft = use_signal(|| value.clone());
    let shown = draft();
    let invalid = parse_hex(shown.trim()).is_none();
    let border = if invalid {
        "border-danger-text"
    } else {
        "border-line"
    };
    rsx! {
        Row { label: t("custom.accent.custom"), help: t("custom.accent.custom.help"),
            div { class: "flex items-center gap-2",
                input {
                    r#type: "color",
                    class: "h-10 w-12 cursor-pointer rounded-md border border-line bg-transparent p-1",
                    "aria-label": t("custom.accent.custom"),
                    value: "{value}",
                    oninput: move |e| {
                        let v = e.value();
                        draft.set(v.clone());
                        on_change.call(v);
                    },
                }
                input {
                    r#type: "text",
                    class: "h-10 w-28 rounded-md border bg-surface-2 px-3 font-mono text-sm text-fg {border}",
                    "aria-label": t("custom.accent.hex"),
                    "aria-invalid": invalid.then_some("true"),
                    maxlength: "7",
                    value: "{shown}",
                    oninput: move |e| {
                        let v = e.value();
                        draft.set(v.clone());
                        if parse_hex(v.trim()).is_some() {
                            on_change.call(v.trim().to_lowercase());
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn GroupTitle(#[props(into)] text: String) -> Element {
    rsx! { h3 { class: "mb-2 mt-7 px-1 text-sm font-semibold text-muted", "{text}" } }
}

#[component]
pub fn Customize(
    settings: std::sync::Arc<Settings>,
    on_change: EventHandler<SettingsChange>,
) -> Element {
    let c = CUSTOM.read().clone();
    let save = move |next: Customization| on_change.call(SettingsChange::Custom(next));
    let preview = move |next: Customization| *CUSTOM.write() = next;

    // One closure pair per slider: change one field of the current value.
    macro_rules! range {
        ($label:expr, $help:expr, $field:ident, $min:expr, $max:expr, $step:expr, $unit:expr) => {{
            let base = c.clone();
            let base2 = c.clone();
            rsx! {
                RangeRow {
                    label: $label,
                    help: $help,
                    value: c.$field,
                    min: $min,
                    max: $max,
                    step: $step,
                    unit: $unit,
                    on_preview: move |v: u8| preview(Customization { $field: v, ..base.clone() }),
                    on_commit: move |v: u8| save(Customization { $field: v, ..base2.clone() }),
                }
            }
        }};
    }
    macro_rules! toggle {
        ($label:expr, $help:expr, $field:ident) => {{
            let base = c.clone();
            rsx! {
                ToggleRow {
                    label: $label,
                    help: $help,
                    checked: c.$field,
                    on_change: move |v: bool| save(Customization { $field: v, ..base.clone() }),
                }
            }
        }};
    }

    let accents = vec![
        TabItem::new("cover", t("custom.accent.mode.cover")),
        TabItem::new("preset", t("custom.accent.mode.preset")),
        TabItem::new("custom", t("custom.accent.mode.custom")),
    ];
    let fonts = vec![
        TabItem::new("inter", "Inter"),
        TabItem::new("jakarta", "Plus Jakarta Sans"),
        TabItem::new("system", t("custom.font.system")),
    ];
    let aligns = vec![
        TabItem::new("left", t("custom.lyrics.align.left")),
        TabItem::new("center", t("custom.lyrics.align.center")),
    ];
    let (b1, b2, b3, b4, b5, b6) = (
        c.clone(),
        c.clone(),
        c.clone(),
        c.clone(),
        c.clone(),
        c.clone(),
    );
    let preset_settings = settings.clone();
    rsx! {
        SectionTitle { title: t("settings.section.customize"), intro: t("custom.intro"), icon: "palette" }

        GroupTitle { text: t("custom.group.color") }
        Card {
            Row { label: t("custom.accent.mode"), help: t("custom.accent.mode.help"),
                SegmentedControl {
                    items: accents,
                    value: accent_value(c.accent_mode),
                    group: "accent-mode",
                    class: "ow-seg",
                    aria_label: t("custom.accent.mode"),
                    on_change: move |v: String| save(Customization { accent_mode: parse_accent(&v), ..b1.clone() }),
                }
            }
            if c.accent_mode == AccentMode::Preset {
                Row { label: t("settings.general.accent"), stack: true,
                    AccentPicker {
                        current: preset_settings.accent.clone(),
                        on_change: move |id: String| on_change.call(SettingsChange::Accent(id)),
                    }
                }
            }
            if c.accent_mode == AccentMode::Custom {
                ColorRow {
                    value: c.custom_accent.clone(),
                    on_change: move |hex: String| save(Customization { custom_accent: hex, ..b2.clone() }),
                }
            }
        }

        GroupTitle { text: t("custom.group.backdrop") }
        Card {
            {toggle!(t("custom.cover_backdrop"), t("custom.cover_backdrop.help"), cover_backdrop)}
            {range!(t("custom.blur"), t("custom.blur.help"), backdrop_blur, 0, BLUR_MAX, 2, " px")}
            {range!(t("custom.brightness"), None::<String>, backdrop_brightness, BRIGHTNESS_RANGE.0, BRIGHTNESS_RANGE.1, 5, "%")}
            {range!(t("custom.dim"), t("custom.dim.help"), backdrop_dim, 0, DIM_MAX, 5, "%")}
        }

        GroupTitle { text: t("custom.group.glass") }
        Card {
            {range!(t("custom.panel_opacity"), t("custom.panel_opacity.help"), panel_opacity, PANEL_OPACITY_RANGE.0, PANEL_OPACITY_RANGE.1, 2, "%")}
            {toggle!(t("custom.panel_border"), None::<String>, panel_border)}
            {range!(t("custom.radius"), None::<String>, radius, 0, RADIUS_MAX, 1, " px")}
            {range!(t("custom.gap"), t("custom.gap.help"), gap, 0, GAP_MAX, 1, " px")}
        }

        GroupTitle { text: t("custom.group.size") }
        Card {
            {range!(t("custom.ui_scale"), t("custom.ui_scale.help"), ui_scale, UI_SCALE_RANGE.0, UI_SCALE_RANGE.1, 5, "%")}
            Row { label: t("custom.font"),
                SegmentedControl {
                    items: fonts,
                    value: font_value(c.font),
                    group: "font",
                    class: "ow-seg",
                    aria_label: t("custom.font"),
                    on_change: move |v: String| save(Customization { font: parse_font(&v), ..b3.clone() }),
                }
            }
            Row { label: t("custom.card_size"), help: t("custom.card_size.help"),
                SegmentedControl {
                    items: size_items(),
                    value: size_value(c.card_size),
                    group: "card-size",
                    class: "ow-seg",
                    aria_label: t("custom.card_size"),
                    on_change: move |v: String| save(Customization { card_size: parse_size(&v), ..b4.clone() }),
                }
            }
            {toggle!(t("custom.animations"), t("custom.animations.help"), animations)}
        }

        GroupTitle { text: t("custom.group.home") }
        Card {
            {toggle!(t("home.recommended"), None::<String>, home_recommended)}
            {toggle!(t("home.mixes"), None::<String>, home_mixes)}
            {toggle!(t("home.discover"), None::<String>, home_discover)}
            {toggle!(t("home.genres"), None::<String>, home_genres)}
            {toggle!(t("home.recent"), None::<String>, home_recent)}
            {toggle!(t("home.liked"), None::<String>, home_liked)}
            {toggle!(t("custom.home_import"), None::<String>, home_import)}
        }

        GroupTitle { text: t("custom.group.sidebar") }
        Card {
            {toggle!(t("custom.sidebar_playlists"), t("custom.sidebar_playlists.help"), sidebar_playlists)}
        }

        GroupTitle { text: t("custom.group.bar") }
        Card {
            {toggle!(t("custom.bar_like"), None::<String>, bar_like)}
            {toggle!(t("custom.bar_shuffle_repeat"), None::<String>, bar_shuffle_repeat)}
            {toggle!(t("player.lyrics"), None::<String>, bar_lyrics)}
            {toggle!(t("player.queue"), None::<String>, bar_queue)}
            {toggle!(t("fx.title"), None::<String>, bar_effects)}
            {toggle!(t("custom.bar_volume"), None::<String>, bar_volume)}
            {toggle!(t("player.fullscreen"), None::<String>, bar_fullscreen)}
            {toggle!(t("sleep.title"), None::<String>, bar_sleep)}
            {toggle!(t("player.mini"), None::<String>, bar_mini)}
            {toggle!(t("player.shortcuts"), None::<String>, bar_help)}
        }

        GroupTitle { text: t("custom.group.fullscreen") }
        Card {
            Row { label: t("custom.fullscreen_art"),
                SegmentedControl {
                    items: size_items(),
                    value: size_value(c.fullscreen_art),
                    group: "fs-art",
                    class: "ow-seg",
                    aria_label: t("custom.fullscreen_art"),
                    on_change: move |v: String| save(Customization { fullscreen_art: parse_size(&v), ..b5.clone() }),
                }
            }
            {range!(t("custom.lyrics_size"), None::<String>, lyrics_size, LYRICS_SIZE_RANGE.0, LYRICS_SIZE_RANGE.1, 5, "%")}
            Row { label: t("custom.lyrics_align"),
                SegmentedControl {
                    items: aligns,
                    value: if c.lyrics_align == LyricsAlign::Center { "center" } else { "left" },
                    group: "lyrics-align",
                    class: "ow-seg",
                    aria_label: t("custom.lyrics_align"),
                    on_change: move |v: String| {
                        let align = if v == "center" { LyricsAlign::Center } else { LyricsAlign::Left };
                        save(Customization { lyrics_align: align, ..b6.clone() })
                    },
                }
            }
        }

        GroupTitle { text: t("custom.css") }
        super::user_css::UserCssCard { saved: settings.user_css.clone(), on_change }

        div { class: "mt-6 flex justify-end",
            Button {
                variant: ButtonVariant::Secondary,
                icon: "refresh-cw",
                disabled: c == Customization::default(),
                onclick: move |_| save(Customization::default()),
                {t("custom.reset")}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_values_round_trip() {
        for m in [AccentMode::Cover, AccentMode::Preset, AccentMode::Custom] {
            assert_eq!(parse_accent(accent_value(m)), m);
        }
        for s in [
            SizeChoice::Small,
            SizeChoice::Medium,
            SizeChoice::Large,
            SizeChoice::XLarge,
        ] {
            assert_eq!(parse_size(size_value(s)), s);
        }
        for f in [FontChoice::Inter, FontChoice::Jakarta, FontChoice::System] {
            assert_eq!(parse_font(font_value(f)), f);
        }
        assert_eq!(parse_size("??"), SizeChoice::Medium);
    }
}
