//! User settings (`settings.json`). Every field tolerates missing or unknown values in the file.
use super::customize::Customization;
use super::versioned::Versioned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Accent colour presets. The UI maps each id to concrete colours.
pub const ACCENT_IDS: [&str; 6] = ["teal", "blue", "violet", "pink", "orange", "green"];
pub const DEFAULT_ACCENT: &str = "teal";
pub const CACHE_LIMIT_RANGE_MB: (u64, u64) = (64, 4096);
pub const DEFAULT_CACHE_LIMIT_MB: u64 = 512;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePref {
    /// Follow the operating system (its language when OpenWave has it, otherwise English).
    #[default]
    System,
    Tr,
    En,
    De,
    Es,
    Fr,
    Pt,
    It,
    Ru,
    Uk,
    Pl,
    Nl,
    Ja,
    Ko,
    Zh,
    Id,
}

impl LanguagePref {
    /// The explicit choice for `lang`.
    pub fn from_lang(lang: crate::i18n::Lang) -> Self {
        use crate::i18n::Lang;
        match lang {
            Lang::Tr => Self::Tr,
            Lang::En => Self::En,
            Lang::De => Self::De,
            Lang::Es => Self::Es,
            Lang::Fr => Self::Fr,
            Lang::Pt => Self::Pt,
            Lang::It => Self::It,
            Lang::Ru => Self::Ru,
            Lang::Uk => Self::Uk,
            Lang::Pl => Self::Pl,
            Lang::Nl => Self::Nl,
            Lang::Ja => Self::Ja,
            Lang::Ko => Self::Ko,
            Lang::Zh => Self::Zh,
            Lang::Id => Self::Id,
        }
    }

    /// The chosen language, or `None` to follow the system.
    pub fn lang(self) -> Option<crate::i18n::Lang> {
        crate::i18n::Lang::ALL
            .into_iter()
            .find(|l| Self::from_lang(*l) == self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePref {
    Light,
    #[default]
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickAction {
    #[default]
    PlayNow,
    AddToQueue,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioQuality {
    Economy,
    #[default]
    High,
}

/// Reverb character of the sound effects. Ids (`snake_case`) match `assets/fx_ir.js`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverbPreset {
    #[default]
    Off,
    Room,
    Salon,
    Hall,
    Cathedral,
    Plate,
}

impl ReverbPreset {
    pub const ALL: [ReverbPreset; 6] = [
        ReverbPreset::Off,
        ReverbPreset::Room,
        ReverbPreset::Salon,
        ReverbPreset::Hall,
        ReverbPreset::Cathedral,
        ReverbPreset::Plate,
    ];

    /// Stable id used by the JS side and in `settings.json`.
    pub fn id(self) -> &'static str {
        match self {
            ReverbPreset::Off => "off",
            ReverbPreset::Room => "room",
            ReverbPreset::Salon => "salon",
            ReverbPreset::Hall => "hall",
            ReverbPreset::Cathedral => "cathedral",
            ReverbPreset::Plate => "plate",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

/// Wet/dry mix of the reverb, in percent.
pub const FX_MIX_MAX: u8 = 100;
pub const DEFAULT_FX_MIX: u8 = 30;
/// Bass boost range in dB (low shelf).
pub const FX_BASS_MAX_DB: u8 = 12;

/// Replace an unreadable value by the field's default instead of failing the whole file.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    Ok(T::deserialize(Value::deserialize(d)?).unwrap_or_default())
}

fn yes() -> bool {
    true
}
fn default_accent() -> String {
    DEFAULT_ACCENT.to_owned()
}
fn default_cache() -> u64 {
    DEFAULT_CACHE_LIMIT_MB
}
fn lenient_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(bool::deserialize(Value::deserialize(d)?).unwrap_or(true))
}
fn lenient_accent<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(String::deserialize(Value::deserialize(d)?).unwrap_or_else(|_| default_accent()))
}
fn lenient_mix<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    Ok(u8::deserialize(Value::deserialize(d)?).unwrap_or(DEFAULT_FX_MIX))
}
fn lenient_cache<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(u64::deserialize(Value::deserialize(d)?).unwrap_or(DEFAULT_CACHE_LIMIT_MB))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(deserialize_with = "lenient")]
    pub language: LanguagePref,
    #[serde(deserialize_with = "lenient")]
    pub theme: ThemePref,
    #[serde(deserialize_with = "lenient_accent")]
    pub accent: String,
    #[serde(deserialize_with = "lenient")]
    pub low_power: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub autoplay_similar: bool,
    #[serde(deserialize_with = "lenient")]
    pub double_click: DoubleClickAction,
    #[serde(deserialize_with = "lenient_true")]
    pub resume_queue: bool,
    #[serde(deserialize_with = "lenient_true")]
    pub pause_on_device_change: bool,
    #[serde(deserialize_with = "lenient")]
    pub audio_quality: AudioQuality,
    /// Not shown any more (nothing fills the cache); kept so settings files round-trip.
    #[serde(deserialize_with = "lenient_cache")]
    pub cache_limit_mb: u64,
    /// Opt-in: fetch lyrics from LRCLIB (sends title/artist/album/duration).
    #[serde(deserialize_with = "lenient")]
    pub lyrics_enabled: bool,
    #[serde(deserialize_with = "lenient")]
    pub onboarding_done: bool,
    /// Sound effects that are remembered between runs. Speed and pitch are not: they start neutral
    /// on every launch so a forgotten "nightcore" never surprises the next session.
    #[serde(deserialize_with = "lenient")]
    pub fx_reverb: ReverbPreset,
    #[serde(deserialize_with = "lenient_mix")]
    pub fx_reverb_mix: u8,
    #[serde(deserialize_with = "lenient")]
    pub fx_bass_db: u8,
    /// "Speed and pitch linked" switch.
    #[serde(deserialize_with = "lenient")]
    pub fx_linked: bool,
    /// Show the current song as "Listening to" in Discord (opt-in; talks only to local Discord).
    #[serde(deserialize_with = "lenient")]
    pub discord_enabled: bool,
    /// Look for a new version on GitHub (and install it on the installed Windows build).
    #[serde(deserialize_with = "lenient_true")]
    pub auto_update: bool,
    /// Look-and-layout choices (Ayarlar › Özelleştirme).
    #[serde(deserialize_with = "lenient")]
    pub custom: Customization,
    /// The user's own Spotify app id, for "Sign in with Spotify" (empty: not set).
    #[serde(deserialize_with = "lenient")]
    pub spotify_client_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LanguagePref::System,
            theme: ThemePref::Dark,
            accent: default_accent(),
            low_power: false,
            autoplay_similar: yes(),
            double_click: DoubleClickAction::PlayNow,
            resume_queue: yes(),
            pause_on_device_change: yes(),
            audio_quality: AudioQuality::High,
            cache_limit_mb: default_cache(),
            lyrics_enabled: false,
            onboarding_done: false,
            fx_reverb: ReverbPreset::Off,
            fx_reverb_mix: DEFAULT_FX_MIX,
            fx_bass_db: 0,
            fx_linked: false,
            discord_enabled: false,
            auto_update: yes(),
            custom: Customization::default(),
            spotify_client_id: String::new(),
        }
    }
}

/// One user-visible change; apply with [`Settings::apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsChange {
    Language(LanguagePref),
    Theme(ThemePref),
    Accent(String),
    LowPower(bool),
    AutoplaySimilar(bool),
    DoubleClick(DoubleClickAction),
    ResumeQueue(bool),
    PauseOnDeviceChange(bool),
    AudioQuality(AudioQuality),
    LyricsEnabled(bool),
    OnboardingDone(bool),
    FxReverb(ReverbPreset),
    FxReverbMix(u8),
    FxBassDb(u8),
    FxLinked(bool),
    DiscordEnabled(bool),
    AutoUpdate(bool),
    Custom(Customization),
    SpotifyClientId(String),
}

impl Settings {
    /// Returns a new value with `change` applied (and normalized).
    pub fn apply(&self, change: SettingsChange) -> Settings {
        let mut next = self.clone();
        match change {
            SettingsChange::Language(v) => next.language = v,
            SettingsChange::Theme(v) => next.theme = v,
            SettingsChange::Accent(v) => next.accent = v,
            SettingsChange::LowPower(v) => next.low_power = v,
            SettingsChange::AutoplaySimilar(v) => next.autoplay_similar = v,
            SettingsChange::DoubleClick(v) => next.double_click = v,
            SettingsChange::ResumeQueue(v) => next.resume_queue = v,
            SettingsChange::PauseOnDeviceChange(v) => next.pause_on_device_change = v,
            SettingsChange::AudioQuality(v) => next.audio_quality = v,
            SettingsChange::LyricsEnabled(v) => next.lyrics_enabled = v,
            SettingsChange::OnboardingDone(v) => next.onboarding_done = v,
            SettingsChange::FxReverb(v) => next.fx_reverb = v,
            SettingsChange::FxReverbMix(v) => next.fx_reverb_mix = v,
            SettingsChange::FxBassDb(v) => next.fx_bass_db = v,
            SettingsChange::FxLinked(v) => next.fx_linked = v,
            SettingsChange::DiscordEnabled(v) => next.discord_enabled = v,
            SettingsChange::AutoUpdate(v) => next.auto_update = v,
            SettingsChange::Custom(v) => next.custom = v,
            SettingsChange::SpotifyClientId(v) => {
                next.spotify_client_id = v.trim().chars().take(64).collect();
            }
        }
        next.sanitize()
    }

    /// Language to display: the explicit choice, or the OS language.
    pub fn effective_language(&self) -> crate::i18n::Lang {
        self.language
            .lang()
            .unwrap_or_else(crate::i18n::detect_system_language)
    }

    pub fn quality(&self) -> crate::backend::api::Quality {
        match self.audio_quality {
            AudioQuality::Economy => crate::backend::api::Quality::Economy,
            AudioQuality::High => crate::backend::api::Quality::High,
        }
    }
}

impl Versioned for Settings {
    const FILE: &'static str = "settings.json";
    const VERSION: u32 = 1;

    /// v0 files used `lang` (a code), `dark` (bool) and `cache_mb`.
    fn migrate(mut doc: Value, from: u32) -> Value {
        let Some(o) = doc.as_object_mut() else {
            return doc;
        };
        if from == 0 {
            if let Some(lang) = o.remove("lang") {
                o.entry("language").or_insert(lang);
            }
            if let Some(dark) = o.remove("dark").and_then(|v| v.as_bool()) {
                o.entry("theme")
                    .or_insert(if dark { "dark" } else { "light" }.into());
            }
            if let Some(mb) = o.remove("cache_mb") {
                o.entry("cache_limit_mb").or_insert(mb);
            }
        }
        doc
    }

    fn sanitize(mut self) -> Self {
        if !ACCENT_IDS.contains(&self.accent.as_str()) {
            self.accent = default_accent();
        }
        let (lo, hi) = CACHE_LIMIT_RANGE_MB;
        self.cache_limit_mb = self.cache_limit_mb.clamp(lo, hi);
        self.fx_reverb_mix = self.fx_reverb_mix.min(FX_MIX_MAX);
        self.fx_bass_db = self.fx_bass_db.min(FX_BASS_MAX_DB);
        self.custom = self.custom.sanitize();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::versioned::Store;

    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ow-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_match_the_spec() {
        let s = Settings::default();
        assert_eq!(
            (s.language, s.theme),
            (LanguagePref::System, ThemePref::Dark)
        );
        assert!(s.autoplay_similar && s.resume_queue && s.pause_on_device_change);
        assert!(!s.lyrics_enabled, "lyrics are opt-in");
        assert!(!s.onboarding_done && !s.low_power);
        assert_eq!(s.cache_limit_mb, 512);
    }

    #[test]
    fn stored_theme_choice_survives_the_new_dark_default() {
        let d = dir();
        std::fs::write(
            d.join("settings.json"),
            br#"{"version":1,"theme":"system"}"#,
        )
        .unwrap();
        assert_eq!(Store::<Settings>::open(&d).get().theme, ThemePref::System);
        let d = dir();
        std::fs::write(d.join("settings.json"), br#"{"version":1,"theme":"light"}"#).unwrap();
        assert_eq!(Store::<Settings>::open(&d).get().theme, ThemePref::Light);
        let d = dir();
        assert_eq!(
            Store::<Settings>::open(&d).get().theme,
            ThemePref::Dark,
            "fresh install"
        );
    }

    #[test]
    fn apply_returns_a_new_value_and_leaves_the_old_one_alone() {
        let a = Settings::default();
        let b = a
            .apply(SettingsChange::Theme(ThemePref::Dark))
            .apply(SettingsChange::LyricsEnabled(true));
        assert_eq!(a.theme, ThemePref::Dark);
        let c = a.apply(SettingsChange::Theme(ThemePref::Light));
        assert_eq!((c.theme, b.lyrics_enabled), (ThemePref::Light, true));
    }

    #[test]
    fn apply_normalizes_accent() {
        let s = Settings::default().apply(SettingsChange::Accent("hotpink".into()));
        assert_eq!(s.accent.as_str(), "teal");
        assert_eq!(
            s.apply(SettingsChange::Accent("violet".into())).accent,
            "violet"
        );
    }

    #[test]
    fn v0_file_with_old_names_and_unknown_fields_migrates() {
        let d = dir();
        std::fs::write(
            d.join("settings.json"),
            br#"{"lang":"en","dark":true,"cache_mb":1024,"legacy_flag":{"a":1},"low_power":true}"#,
        )
        .unwrap();
        let store = Store::<Settings>::open(&d);
        let s = store.get();
        assert_eq!((s.language, s.theme), (LanguagePref::En, ThemePref::Dark));
        assert_eq!((s.cache_limit_mb, s.low_power), (1024, true));
        assert!(s.autoplay_similar, "missing fields take defaults");
        store
            .update(|s| s.apply(SettingsChange::OnboardingDone(true)))
            .unwrap();
        store.flush().unwrap();
        let raw = std::fs::read_to_string(d.join("settings.json")).unwrap();
        assert!(
            raw.contains("\"version\":1")
                && !raw.contains("\"lang\"")
                && !raw.contains("legacy_flag")
        );
    }

    #[test]
    fn bad_values_fall_back_per_field_without_losing_the_rest() {
        let d = dir();
        std::fs::write(
            d.join("settings.json"),
            br#"{"version":1,"theme":"neon","language":42,"autoplay_similar":"maybe","cache_limit_mb":-5,"accent":7,"low_power":true}"#,
        )
        .unwrap();
        let store = Store::<Settings>::open(&d);
        let s = store.get();
        assert_eq!(
            s.theme,
            ThemePref::Dark,
            "a bad value falls back to the default look"
        );
        assert_eq!(s.language, LanguagePref::System);
        assert!(s.autoplay_similar);
        assert_eq!(s.cache_limit_mb, 512);
        assert_eq!(s.accent, "teal");
        assert!(s.low_power, "valid fields survive");
        assert!(store.report().recovered_from.is_none());
    }

    #[test]
    fn unknown_future_enum_values_do_not_crash() {
        let d = dir();
        std::fs::write(
            d.join("settings.json"),
            br#"{"version":1,"audio_quality":"lossless","double_click":"explode"}"#,
        )
        .unwrap();
        let s = Store::<Settings>::open(&d).get();
        assert_eq!(
            (s.audio_quality, s.double_click),
            (AudioQuality::High, DoubleClickAction::PlayNow)
        );
    }

    #[test]
    fn corrupt_settings_start_clean_and_keep_a_backup() {
        let d = dir();
        std::fs::write(d.join("settings.json"), b"{\"theme\": \"dark\"").unwrap();
        let store = Store::<Settings>::open(&d);
        assert_eq!(*store.get(), Settings::default());
        assert!(store
            .report()
            .recovered_from
            .as_ref()
            .is_some_and(|p| p.is_file()));
    }

    #[test]
    fn explicit_language_wins_over_system() {
        assert_eq!(
            Settings::default()
                .apply(SettingsChange::Language(LanguagePref::Tr))
                .effective_language(),
            crate::i18n::Lang::Tr
        );
        assert_eq!(
            Settings::default()
                .apply(SettingsChange::Language(LanguagePref::En))
                .effective_language(),
            crate::i18n::Lang::En
        );
    }

    #[test]
    fn effect_fields_default_off_and_old_files_still_load() {
        let s = Settings::default();
        assert_eq!(
            (s.fx_reverb, s.fx_reverb_mix, s.fx_bass_db, s.fx_linked),
            (ReverbPreset::Off, 30, 0, false)
        );
        // A settings.json written before the effects existed has none of the fields.
        let d = dir();
        std::fs::write(d.join("settings.json"), br#"{"version":1,"theme":"dark"}"#).unwrap();
        let s = Store::<Settings>::open(&d).get();
        assert_eq!(s.theme, ThemePref::Dark);
        assert_eq!((s.fx_reverb, s.fx_reverb_mix), (ReverbPreset::Off, 30));
    }

    #[test]
    fn effect_fields_survive_bad_and_out_of_range_values() {
        let d = dir();
        std::fs::write(
            d.join("settings.json"),
            br#"{"version":1,"fx_reverb":"swamp","fx_reverb_mix":999,"fx_bass_db":-4,"fx_linked":"yes","low_power":true}"#,
        )
        .unwrap();
        let s = Store::<Settings>::open(&d).get();
        assert_eq!(s.fx_reverb, ReverbPreset::Off);
        assert_eq!(
            s.fx_reverb_mix, 30,
            "unreadable (> u8) falls back to the default"
        );
        assert_eq!(s.fx_bass_db, 0);
        assert!(!s.fx_linked && s.low_power);
        let s = Settings::default()
            .apply(SettingsChange::FxReverbMix(250))
            .apply(SettingsChange::FxBassDb(99))
            .apply(SettingsChange::FxReverb(ReverbPreset::Hall));
        assert_eq!((s.fx_reverb_mix, s.fx_bass_db), (100, 12));
        assert_eq!(s.fx_reverb.id(), "hall");
    }

    #[test]
    fn reverb_ids_round_trip() {
        for p in ReverbPreset::ALL {
            assert_eq!(ReverbPreset::from_id(p.id()), Some(p));
            assert_eq!(
                serde_json::to_value(p).unwrap(),
                serde_json::Value::String(p.id().into())
            );
        }
        assert_eq!(ReverbPreset::from_id("nope"), None);
    }

    #[test]
    fn discord_switch_and_customization_are_sanitized() {
        let s = Settings::default().apply(SettingsChange::DiscordEnabled(true));
        assert!(s.discord_enabled);
        // Files written while the id was a setting still load.
        let old: Settings =
            serde_json::from_str(r#"{"discord_enabled":true,"discord_app_id":"123"}"#).unwrap();
        assert!(old.discord_enabled);
        let custom = Customization {
            radius: 200,
            ..Customization::default()
        };
        let c = Settings::default().apply(SettingsChange::Custom(custom));
        assert_eq!(c.custom.radius, crate::store::customize::RADIUS_MAX);
    }
}
