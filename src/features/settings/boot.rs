//! Maps saved settings onto the running app (theme, accent, low power, language) and applies
//! them at startup and after every change, so nothing needs a restart.

use std::sync::Arc;

use dioxus::prelude::*;

use crate::i18n::{self, Lang};
use crate::store::customize::Customization;
use crate::store::{self, Settings, SettingsChange, ThemePref};
use crate::ui::{toast_error, Accent, Theme, ACCENT, LOW_POWER, THEME};

pub fn theme_of(pref: ThemePref) -> Theme {
    match pref {
        ThemePref::Light => Theme::Light,
        ThemePref::Dark => Theme::Dark,
        ThemePref::System => Theme::System,
    }
}

/// Stored accent ids use "pink"; the CSS preset for it is called "rose".
pub fn accent_of(id: &str) -> Accent {
    match id {
        "pink" => Accent::Rose,
        other => Accent::parse(other),
    }
}

/// Everything the running app derives from the saved settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Runtime {
    pub theme: Theme,
    pub accent: Accent,
    pub low_power: bool,
    pub lang: Lang,
}

pub fn runtime_of(s: &Settings) -> Runtime {
    Runtime {
        theme: theme_of(s.theme),
        accent: accent_of(&s.accent),
        low_power: s.low_power,
        lang: s.effective_language(),
    }
}

/// The Discord status switch, as saved (read by `features::discord`).
pub static DISCORD: GlobalSignal<bool> = Signal::global(|| false);
/// The saved customization (read by the layout, Home, the player bar and `App`'s style sync).
pub static CUSTOM: GlobalSignal<Customization> = Signal::global(Customization::default);

/// JS that installs the customization stylesheet and `data-*` switches, then repaints the cover
/// colours (so switching between a fixed and a cover-driven accent takes effect at once).
pub fn custom_script(c: &Customization) -> String {
    use crate::store::customize::{custom_css, custom_flags};
    let css = serde_json::to_string(&custom_css(c)).unwrap_or_else(|_| "\"\"".into());
    let flags: Vec<(&str, bool)> = custom_flags(c).to_vec();
    let flags = serde_json::to_string(&flags).unwrap_or_else(|_| "[]".into());
    format!(
        "(function(css,flags){{var s=document.getElementById('ow-custom');if(!s){{s=document.createElement('style');s.id='ow-custom';document.head.appendChild(s);}}s.textContent=css;var r=document.documentElement;flags.forEach(function(f){{if(f[1])r.setAttribute(f[0],'');else r.removeAttribute(f[0]);}});if(window.__owTheme&&window.__owTheme.refresh)window.__owTheme.refresh();}})({css},{flags});"
    )
}

/// Call once near the root: keeps the customization stylesheet in sync with [`CUSTOM`].
pub fn use_custom_style() {
    use_effect(move || {
        document::eval(&custom_script(&CUSTOM.read()));
    });
}

/// Push the non-`Copy` parts of the settings (Discord, customization) into their signals.
pub fn apply_extras(s: &Settings) {
    if *DISCORD.peek() != s.discord_enabled {
        *DISCORD.write() = s.discord_enabled;
    }
    if *CUSTOM.peek() != s.custom {
        *CUSTOM.write() = s.custom.clone();
    }
}

fn set_if_changed<T: PartialEq + Copy + 'static>(signal: &'static GlobalSignal<T>, value: T) {
    if *signal.peek() != value {
        *signal.write() = value;
    }
}

/// Push `rt` into the UI state. `skip_look` leaves theme/accent/low-power alone (debug env).
pub fn apply_runtime(rt: Runtime, skip_look: bool) {
    if !skip_look {
        set_if_changed(&THEME, rt.theme);
        set_if_changed(&ACCENT, rt.accent);
        set_if_changed(&LOW_POWER, rt.low_power);
    }
    if i18n::language() != rt.lang {
        i18n::set_language(rt.lang);
    }
}

#[cfg(debug_assertions)]
fn debug_overrides_look() -> bool {
    ["OPENWAVE_THEME", "OPENWAVE_ACCENT", "OPENWAVE_LOW_POWER"]
        .iter()
        .any(|k| std::env::var_os(k).is_some())
}

#[cfg(not(debug_assertions))]
fn debug_overrides_look() -> bool {
    false
}

/// Apply the saved settings once at startup.
pub fn apply_stored() {
    let s = store::settings().get();
    apply_runtime(runtime_of(&s), debug_overrides_look());
    apply_extras(&s);
}

/// Save one change, apply it live, and return the new snapshot. A failed write is reported
/// to the user but the change still applies for this session.
pub fn commit(change: SettingsChange) -> Arc<Settings> {
    let store = store::settings();
    if let Err(e) = store.update(|s| s.apply(change.clone())) {
        use crate::backend::error::Coded;
        toast_error(e.user_message());
    }
    let now = store.get();
    apply_runtime(runtime_of(&now), false);
    apply_extras(&now);
    now
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::LanguagePref;

    #[test]
    fn theme_prefs_map_one_to_one() {
        assert_eq!(theme_of(ThemePref::Light), Theme::Light);
        assert_eq!(theme_of(ThemePref::Dark), Theme::Dark);
        assert_eq!(theme_of(ThemePref::System), Theme::System);
    }

    #[test]
    fn every_stored_accent_id_has_its_own_preset() {
        let mut seen = vec![];
        for id in store::ACCENT_IDS {
            let a = accent_of(id);
            assert!(!seen.contains(&a), "{id} collides");
            seen.push(a);
        }
        assert_eq!(accent_of("pink"), Accent::Rose);
        assert_eq!(accent_of("blue"), Accent::Blue);
        assert_eq!(accent_of("nonsense"), Accent::Teal);
    }

    #[test]
    fn settings_changes_map_to_runtime_state() {
        let s = Settings::default()
            .apply(SettingsChange::Theme(ThemePref::Dark))
            .apply(SettingsChange::Accent("violet".into()))
            .apply(SettingsChange::LowPower(true))
            .apply(SettingsChange::Language(LanguagePref::Tr));
        assert_eq!(
            runtime_of(&s),
            Runtime {
                theme: Theme::Dark,
                accent: Accent::Violet,
                low_power: true,
                lang: Lang::Tr
            }
        );
        let en = s.apply(SettingsChange::Language(LanguagePref::En));
        assert_eq!(runtime_of(&en).lang, Lang::En);
    }

    #[test]
    fn custom_script_embeds_css_as_a_json_string_and_sets_flags() {
        let c = Customization {
            cover_backdrop: false,
            ..Customization::default()
        };
        let js = custom_script(&c);
        assert!(js.contains("--ow-bd-blur:56px"));
        assert!(js.contains("[\"data-no-cover-bg\",true]"));
        assert!(js.starts_with("(function(css,flags){"));
    }
}
