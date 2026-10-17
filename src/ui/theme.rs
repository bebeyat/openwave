//! Theme, accent and low-power state. Settings screens write the global signals;
//! `use_theme_sync` (called once in `App`) mirrors them onto `<html>` attributes.

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    /// Follow the OS (prefers-color-scheme).
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    /// Value of `data-theme`, or `None` to follow the system.
    pub fn attr(self) -> Option<&'static str> {
        match self {
            Theme::System => None,
            Theme::Light => Some("light"),
            Theme::Dark => Some("dark"),
        }
    }

    #[cfg(debug_assertions)]
    pub fn parse(s: &str) -> Self {
        match s {
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            _ => Theme::System,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Accent {
    #[default]
    Teal,
    Blue,
    Violet,
    Rose,
    Orange,
    Green,
}

impl Accent {
    pub const ALL: [Accent; 6] = [
        Accent::Teal,
        Accent::Blue,
        Accent::Violet,
        Accent::Rose,
        Accent::Orange,
        Accent::Green,
    ];

    pub fn attr(self) -> &'static str {
        match self {
            Accent::Teal => "teal",
            Accent::Blue => "blue",
            Accent::Violet => "violet",
            Accent::Rose => "rose",
            Accent::Orange => "orange",
            Accent::Green => "green",
        }
    }

    pub fn parse(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|a| a.attr() == s)
            .unwrap_or_default()
    }
}

pub static THEME: GlobalSignal<Theme> = Signal::global(Theme::default);
pub static ACCENT: GlobalSignal<Accent> = Signal::global(Accent::default);
/// "Düşük güç modu": removes decorative motion (`html[data-low-power]`).
pub static LOW_POWER: GlobalSignal<bool> = Signal::global(|| false);

/// Builds the JS that applies the three settings to `<html>`.
pub fn apply_script(theme: Theme, accent: Accent, low_power: bool) -> String {
    let set_theme = match theme.attr() {
        Some(v) => format!("r.setAttribute('data-theme','{v}');"),
        None => "r.removeAttribute('data-theme');".to_string(),
    };
    let power = if low_power {
        "r.setAttribute('data-low-power','');"
    } else {
        "r.removeAttribute('data-low-power');"
    };
    format!(
        "(function(){{var r=document.documentElement;{set_theme}r.setAttribute('data-accent','{}');{power}}})();",
        accent.attr()
    )
}

/// Call once near the root: keeps `<html data-theme/data-accent/data-low-power>` in sync.
pub fn use_theme_sync() {
    use_effect(move || {
        let script = apply_script(THEME(), ACCENT(), LOW_POWER());
        document::eval(&script);
    });
}

/// Duration (ms) to keep an exiting overlay mounted so its exit animation can play.
pub fn exit_ms(base: u64) -> u64 {
    if *LOW_POWER.peek() {
        0
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_theme_removes_attribute() {
        let s = apply_script(Theme::System, Accent::Teal, false);
        assert!(s.contains("removeAttribute('data-theme')"));
        assert!(s.contains("removeAttribute('data-low-power')"));
    }

    #[test]
    fn explicit_theme_and_accent() {
        let s = apply_script(Theme::Dark, Accent::Rose, true);
        assert!(s.contains("data-theme','dark'"));
        assert!(s.contains("data-accent','rose'"));
        assert!(s.contains("setAttribute('data-low-power'"));
    }

    #[test]
    fn parse_round_trips() {
        for a in Accent::ALL {
            assert_eq!(Accent::parse(a.attr()), a);
        }
        assert_eq!(Theme::parse("dark"), Theme::Dark);
        assert_eq!(Theme::parse("x"), Theme::System);
    }
}
