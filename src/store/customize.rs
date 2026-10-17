//! Look-and-layout customization (Ayarlar › Özelleştirme), stored as `custom` in `settings.json`.
//!
//! Everything here is cosmetic and applied live: colours, backdrop, glass, sizes, motion, which
//! Home sections and player-bar controls show, lyrics and fullscreen artwork. `custom_css` turns
//! the values into the CSS variables and `data-*` switches the stylesheet reads.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentMode {
    /// The accent follows the current cover (falls back to the preset with nothing playing).
    #[default]
    Cover,
    /// Always the chosen preset.
    Preset,
    /// Always `custom_accent`.
    Custom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontChoice {
    #[default]
    Inter,
    Jakarta,
    System,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeChoice {
    Small,
    #[default]
    Medium,
    Large,
    XLarge,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsAlign {
    #[default]
    Left,
    Center,
}

pub const DEFAULT_CUSTOM_ACCENT: &str = "#35d0b0";
pub const BLUR_MAX: u8 = 120;
pub const DIM_MAX: u8 = 90;
pub const BRIGHTNESS_RANGE: (u8, u8) = (30, 160);
pub const PANEL_OPACITY_RANGE: (u8, u8) = (10, 100);
pub const RADIUS_MAX: u8 = 28;
pub const GAP_MAX: u8 = 24;
pub const UI_SCALE_RANGE: (u8, u8) = (80, 130);
pub const LYRICS_SIZE_RANGE: (u8, u8) = (60, 160);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Customization {
    // Colour
    pub accent_mode: AccentMode,
    pub custom_accent: String,
    // Backdrop
    pub cover_backdrop: bool,
    /// Blur of the cover behind the panels, in px.
    pub backdrop_blur: u8,
    /// Extra darkening over the backdrop, in percent.
    pub backdrop_dim: u8,
    /// Brightness of the backdrop, in percent of the theme's own.
    pub backdrop_brightness: u8,
    // Glass and shape
    /// Panel opacity in percent (100 = solid).
    pub panel_opacity: u8,
    pub panel_border: bool,
    /// Corner radius of panels and cards, in px.
    pub radius: u8,
    /// Space between the floating panels, in px.
    pub gap: u8,
    // Text and size
    /// Whole-interface text scale, in percent.
    pub ui_scale: u8,
    pub font: FontChoice,
    pub card_size: SizeChoice,
    pub animations: bool,
    // Home sections
    pub home_recommended: bool,
    pub home_mixes: bool,
    pub home_genres: bool,
    pub home_recent: bool,
    pub home_liked: bool,
    pub home_import: bool,
    // Sidebar
    pub sidebar_playlists: bool,
    // Player bar controls
    pub bar_like: bool,
    pub bar_shuffle_repeat: bool,
    pub bar_lyrics: bool,
    pub bar_queue: bool,
    pub bar_effects: bool,
    pub bar_volume: bool,
    pub bar_fullscreen: bool,
    pub bar_help: bool,
    // Fullscreen player and lyrics
    pub fullscreen_art: SizeChoice,
    /// Lyrics text size, in percent.
    pub lyrics_size: u8,
    pub lyrics_align: LyricsAlign,
}

impl Default for Customization {
    fn default() -> Self {
        Self {
            accent_mode: AccentMode::Cover,
            custom_accent: DEFAULT_CUSTOM_ACCENT.to_owned(),
            cover_backdrop: true,
            backdrop_blur: 56,
            backdrop_dim: 0,
            backdrop_brightness: 100,
            panel_opacity: 46,
            panel_border: true,
            radius: 12,
            gap: 8,
            ui_scale: 100,
            font: FontChoice::Inter,
            card_size: SizeChoice::Medium,
            animations: true,
            home_recommended: true,
            home_mixes: true,
            home_genres: true,
            home_recent: true,
            home_liked: true,
            home_import: true,
            sidebar_playlists: true,
            bar_like: true,
            bar_shuffle_repeat: true,
            bar_lyrics: true,
            bar_queue: true,
            bar_effects: true,
            bar_volume: true,
            bar_fullscreen: true,
            bar_help: true,
            fullscreen_art: SizeChoice::Large,
            lyrics_size: 100,
            lyrics_align: LyricsAlign::Left,
        }
    }
}

/// `#rrggbb` (case-insensitive), the only colour form accepted from the user.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn clamp(v: u8, (lo, hi): (u8, u8)) -> u8 {
    v.clamp(lo, hi)
}

impl Customization {
    /// Every value back inside its range; an unreadable colour becomes the default one.
    pub fn sanitize(mut self) -> Self {
        if parse_hex(&self.custom_accent).is_none() {
            self.custom_accent = DEFAULT_CUSTOM_ACCENT.to_owned();
        }
        self.custom_accent = self.custom_accent.to_lowercase();
        self.backdrop_blur = self.backdrop_blur.min(BLUR_MAX);
        self.backdrop_dim = self.backdrop_dim.min(DIM_MAX);
        self.backdrop_brightness = clamp(self.backdrop_brightness, BRIGHTNESS_RANGE);
        self.panel_opacity = clamp(self.panel_opacity, PANEL_OPACITY_RANGE);
        self.radius = self.radius.min(RADIUS_MAX);
        self.gap = self.gap.min(GAP_MAX);
        self.ui_scale = clamp(self.ui_scale, UI_SCALE_RANGE);
        self.lyrics_size = clamp(self.lyrics_size, LYRICS_SIZE_RANGE);
        self
    }
}

/// WCAG relative luminance of an sRGB colour.
fn luminance([r, g, b]: [u8; 3]) -> f64 {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Text colour on an accent fill: near-black on light accents, white on dark ones.
pub fn on_accent(c: [u8; 3]) -> &'static str {
    if luminance(c) > 0.18 {
        "#0c0b0a"
    } else {
        "#ffffff"
    }
}

/// Mix `c` toward `to` by `t` (0..1).
fn mix(c: [u8; 3], to: [u8; 3], t: f64) -> [u8; 3] {
    let m = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Card width per size choice (min, viewport share, max).
fn card_clamp(size: SizeChoice) -> &'static str {
    match size {
        SizeChoice::Small => "clamp(128px, 7.5vw, 168px)",
        SizeChoice::Medium => "clamp(152px, 9.5vw, 216px)",
        SizeChoice::Large => "clamp(184px, 12vw, 264px)",
        SizeChoice::XLarge => "clamp(216px, 15vw, 320px)",
    }
}

fn fullscreen_art_px(size: SizeChoice) -> u32 {
    match size {
        SizeChoice::Small => 220,
        SizeChoice::Medium => 280,
        SizeChoice::Large => 340,
        SizeChoice::XLarge => 440,
    }
}

fn font_stack(font: FontChoice) -> Option<&'static str> {
    match font {
        FontChoice::Inter => None,
        FontChoice::Jakarta => {
            Some("\"Plus Jakarta Sans Variable\", \"Inter Variable\", sans-serif")
        }
        FontChoice::System => Some("\"Segoe UI Variable\", \"Segoe UI\", system-ui, sans-serif"),
    }
}

/// The stylesheet text for `<style id="ow-custom">`. Only numbers, fixed keywords and a
/// validated `#rrggbb` reach it, so user input can never inject CSS.
pub fn custom_css(c: &Customization) -> String {
    let c = c.clone().sanitize();
    let mut css = format!(
        ":root{{--ow-bd-blur:{}px;--ow-bd-dim:{:.2};--ow-bd-mult:{:.2};--ow-panel-a:{:.2};\
         --radius-md:{}px;--radius-lg:{}px;--radius-xl:{}px;--ow-gap:{}px;--ow-ui-scale:{:.2};\
         --ow-card-w:{};--ow-fs-art:{}px;--ow-lyric-scale:{:.2};}}",
        c.backdrop_blur,
        f64::from(c.backdrop_dim) / 100.0,
        f64::from(c.backdrop_brightness) / 100.0,
        f64::from(c.panel_opacity) / 100.0,
        (u32::from(c.radius) * 2 / 3).max(2),
        (u32::from(c.radius) * 5 / 6).max(2),
        c.radius,
        c.gap,
        f64::from(c.ui_scale) / 100.0,
        card_clamp(c.card_size),
        fullscreen_art_px(c.fullscreen_art),
        f64::from(c.lyrics_size) / 100.0,
    );
    if c.accent_mode == AccentMode::Custom {
        if let Some(rgb) = parse_hex(&c.custom_accent) {
            css.push_str(&format!(
                ":root{{--ow-accent:{a}!important;--ow-accent-2:{a}!important;--ow-on-accent:{on}!important;\
                 --ow-at-d:{d}!important;--ow-at-l:{l}!important;}}",
                a = hex(rgb),
                on = on_accent(rgb),
                d = hex(mix(rgb, [255, 255, 255], 0.18)),
                l = hex(mix(rgb, [0, 0, 0], 0.45)),
            ));
        }
    }
    if let Some(stack) = font_stack(c.font) {
        css.push_str(&format!(
            ":root{{--font-sans:{stack};--font-display:{stack};}}"
        ));
    }
    css
}

/// `data-*` switches on `<html>`, as (name, on) pairs.
pub fn custom_flags(c: &Customization) -> [(&'static str, bool); 5] {
    [
        ("data-fixed-accent", c.accent_mode != AccentMode::Cover),
        ("data-no-cover-bg", !c.cover_backdrop),
        ("data-no-border", !c.panel_border),
        ("data-no-motion", !c.animations),
        ("data-lyrics-center", c.lyrics_align == LyricsAlign::Center),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex("#35D0b0"), Some([0x35, 0xd0, 0xb0]));
        for bad in ["35d0b0", "#35d0b", "#35d0b0ff", "#zzzzzz", "", "#;}{a}"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn sanitize_clamps_everything() {
        let c = Customization {
            custom_accent: "red;}".into(),
            backdrop_blur: 255,
            backdrop_dim: 255,
            backdrop_brightness: 0,
            panel_opacity: 0,
            radius: 99,
            gap: 99,
            ui_scale: 255,
            lyrics_size: 1,
            ..Customization::default()
        }
        .sanitize();
        assert_eq!(c.custom_accent, DEFAULT_CUSTOM_ACCENT);
        assert_eq!(
            (
                c.backdrop_blur,
                c.backdrop_dim,
                c.backdrop_brightness,
                c.panel_opacity
            ),
            (BLUR_MAX, DIM_MAX, BRIGHTNESS_RANGE.0, PANEL_OPACITY_RANGE.0)
        );
        assert_eq!(
            (c.radius, c.gap, c.ui_scale, c.lyrics_size),
            (RADIUS_MAX, GAP_MAX, 130, 60)
        );
    }

    #[test]
    fn css_carries_values_and_only_a_custom_accent_overrides_colours() {
        let base = custom_css(&Customization::default());
        assert!(base.contains("--ow-bd-blur:56px") && base.contains("--ow-panel-a:0.46"));
        assert!(base.contains("--radius-xl:12px") && base.contains("--ow-gap:8px"));
        assert!(
            !base.contains("--ow-accent:"),
            "cover mode leaves the accent alone"
        );
        let custom = custom_css(&Customization {
            accent_mode: AccentMode::Custom,
            custom_accent: "#ffcc00".into(),
            font: FontChoice::System,
            ..Customization::default()
        });
        assert!(custom.contains("--ow-accent:#ffcc00!important"));
        assert!(
            custom.contains("--ow-on-accent:#0c0b0a"),
            "dark text on yellow"
        );
        assert!(custom.contains("--font-sans:\"Segoe UI Variable\""));
    }

    #[test]
    fn hostile_colour_never_reaches_the_stylesheet() {
        let css = custom_css(&Customization {
            accent_mode: AccentMode::Custom,
            custom_accent: "#000}body{display:none".into(),
            ..Customization::default()
        });
        assert!(!css.contains("display:none"));
        assert!(css.contains(&format!("--ow-accent:{DEFAULT_CUSTOM_ACCENT}")));
    }

    #[test]
    fn on_accent_picks_readable_text() {
        assert_eq!(on_accent([255, 255, 255]), "#0c0b0a");
        assert_eq!(on_accent([20, 20, 80]), "#ffffff");
    }

    #[test]
    fn flags_follow_the_switches() {
        let flags = custom_flags(&Customization {
            accent_mode: AccentMode::Preset,
            cover_backdrop: false,
            animations: false,
            ..Customization::default()
        });
        let on: Vec<&str> = flags.iter().filter(|(_, v)| *v).map(|(k, _)| *k).collect();
        assert_eq!(
            on,
            ["data-fixed-accent", "data-no-cover-bg", "data-no-motion"]
        );
    }

    #[test]
    fn missing_and_broken_fields_fall_back() {
        let c: Customization = serde_json::from_str(r#"{"radius": 20}"#).unwrap();
        assert_eq!(c.radius, 20);
        assert!(c.home_genres && c.bar_volume);
    }
}
