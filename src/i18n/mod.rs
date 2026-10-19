//! Translation lookup. Contract used by every screen:
//!
//! * `t("section.name")` – text for the current language
//! * `t_args("section.name", &[("count", "3")])` – replaces `{count}` placeholders
//! * `t_count("section.songs", 3)` – picks `section.songs.one` / `.other` and fills `{count}`
//!
//! Keys live in `assets/locales/<code>.json` (15 languages, see [`Lang::ALL`]) as flat
//! `"section.name": "text"` maps, embedded at compile time. Lookup falls back to English and
//! finally to the key itself, so a missing translation is visible but never a crash. Add a key to
//! EVERY file; a test enforces it, and another checks that every literal key used in
//! `src/**/*.rs` exists. Plural keys are `<key>.one/.other`, plus `.few/.many` where the language
//! needs them (Russian, Ukrainian, Polish).
//!
//! Switching language: call [`set_language`]. Components that call `t()` re-render because every
//! lookup subscribes to [`LANGUAGE_REVISION`], a Dioxus global signal bumped by `set_language`.
// The UI consumes this API incrementally; unused items are expected until it does.

use dioxus::dioxus_core::Runtime;
use dioxus::prelude::{GlobalSignal, ReadableExt, Signal};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        OnceLock,
    },
};

const UNSET: u8 = u8::MAX;

/// Every supported language with its embedded locale file, in menu order.
const SOURCES: [(Lang, &str); 15] = [
    (Lang::Tr, include_str!("../../assets/locales/tr.json")),
    (Lang::En, include_str!("../../assets/locales/en.json")),
    (Lang::De, include_str!("../../assets/locales/de.json")),
    (Lang::Es, include_str!("../../assets/locales/es.json")),
    (Lang::Fr, include_str!("../../assets/locales/fr.json")),
    (Lang::Pt, include_str!("../../assets/locales/pt.json")),
    (Lang::It, include_str!("../../assets/locales/it.json")),
    (Lang::Ru, include_str!("../../assets/locales/ru.json")),
    (Lang::Uk, include_str!("../../assets/locales/uk.json")),
    (Lang::Pl, include_str!("../../assets/locales/pl.json")),
    (Lang::Nl, include_str!("../../assets/locales/nl.json")),
    (Lang::Ja, include_str!("../../assets/locales/ja.json")),
    (Lang::Ko, include_str!("../../assets/locales/ko.json")),
    (Lang::Zh, include_str!("../../assets/locales/zh.json")),
    (Lang::Id, include_str!("../../assets/locales/id.json")),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    Tr,
    #[default]
    En,
    De,
    Es,
    Fr,
    /// Brazilian Portuguese.
    Pt,
    It,
    Ru,
    Uk,
    Pl,
    Nl,
    Ja,
    Ko,
    /// Simplified Chinese.
    Zh,
    Id,
}

impl Lang {
    pub const ALL: [Lang; 15] = [
        Lang::Tr,
        Lang::En,
        Lang::De,
        Lang::Es,
        Lang::Fr,
        Lang::Pt,
        Lang::It,
        Lang::Ru,
        Lang::Uk,
        Lang::Pl,
        Lang::Nl,
        Lang::Ja,
        Lang::Ko,
        Lang::Zh,
        Lang::Id,
    ];

    /// Two-letter code (`"tr"`, `"en"`, …), also the locale file name.
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tr => "tr",
            Lang::En => "en",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::Pt => "pt",
            Lang::It => "it",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
            Lang::Pl => "pl",
            Lang::Nl => "nl",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Zh => "zh",
            Lang::Id => "id",
        }
    }

    /// The language's own name, as shown in the language menu.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Tr => "Türkçe",
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::Pt => "Português (Brasil)",
            Lang::It => "Italiano",
            Lang::Ru => "Русский",
            Lang::Uk => "Українська",
            Lang::Pl => "Polski",
            Lang::Nl => "Nederlands",
            Lang::Ja => "日本語",
            Lang::Ko => "한국어",
            Lang::Zh => "简体中文",
            Lang::Id => "Bahasa Indonesia",
        }
    }

    /// The language's flag (SVG from flag-icons, see `tools/extract-assets.mjs`) for the language
    /// menu: Windows has no colour emoji flags.
    pub fn flag_svg(self) -> &'static str {
        match self {
            Lang::Tr => include_str!("../../assets/flags/tr.svg"),
            Lang::En => include_str!("../../assets/flags/en.svg"),
            Lang::De => include_str!("../../assets/flags/de.svg"),
            Lang::Es => include_str!("../../assets/flags/es.svg"),
            Lang::Fr => include_str!("../../assets/flags/fr.svg"),
            Lang::Pt => include_str!("../../assets/flags/pt.svg"),
            Lang::It => include_str!("../../assets/flags/it.svg"),
            Lang::Ru => include_str!("../../assets/flags/ru.svg"),
            Lang::Uk => include_str!("../../assets/flags/uk.svg"),
            Lang::Pl => include_str!("../../assets/flags/pl.svg"),
            Lang::Nl => include_str!("../../assets/flags/nl.svg"),
            Lang::Ja => include_str!("../../assets/flags/ja.svg"),
            Lang::Ko => include_str!("../../assets/flags/ko.svg"),
            Lang::Zh => include_str!("../../assets/flags/zh.svg"),
            Lang::Id => include_str!("../../assets/flags/id.svg"),
        }
    }

    /// Parse `tr`, `tr-TR`, `en_US`, `pt-BR`, … (case-insensitive); anything else is `None`.
    pub fn from_code(code: &str) -> Option<Lang> {
        let primary = code
            .trim()
            .split(['-', '_', '.'])
            .next()?
            .to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    fn index(self) -> u8 {
        Lang::ALL.iter().position(|l| *l == self).unwrap_or(1) as u8
    }
}

/// CLDR plural category of `n` in `lang`: `one`, `few`, `many` or `other`.
pub fn plural_category(lang: Lang, n: usize) -> &'static str {
    let (m10, m100) = (n % 10, n % 100);
    match lang {
        Lang::Ru | Lang::Uk => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        Lang::Pl => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        Lang::Fr | Lang::Pt => {
            if n <= 1 {
                "one"
            } else {
                "other"
            }
        }
        Lang::Ja | Lang::Ko | Lang::Zh | Lang::Id => "other",
        Lang::Tr | Lang::En | Lang::De | Lang::Es | Lang::It | Lang::Nl => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// Bumped on every language change; reading it inside a component subscribes that component.
pub static LANGUAGE_REVISION: GlobalSignal<u64> = Signal::global(|| 0);

static CURRENT: AtomicU8 = AtomicU8::new(UNSET);

/// The OS language when OpenWave has it, English otherwise.
pub fn detect_system_language() -> Lang {
    sys_locale::get_locale()
        .and_then(|l| Lang::from_code(&l))
        .unwrap_or(Lang::En)
}

/// Language currently shown (the OS language until [`set_language`] is called).
pub fn language() -> Lang {
    match Lang::ALL.get(usize::from(CURRENT.load(Ordering::Relaxed))) {
        Some(lang) => *lang,
        None => detect_system_language(),
    }
}

/// Switch language at runtime. Safe to call from anywhere; components refresh when called on the
/// UI thread (inside the Dioxus runtime).
pub fn set_language(lang: Lang) {
    CURRENT.store(lang.index(), Ordering::Relaxed);
    if Runtime::try_current().is_some() {
        *LANGUAGE_REVISION.write() += 1;
    }
}

/// The embedded locale text of `lang`.
pub(crate) fn source(lang: Lang) -> &'static str {
    SOURCES
        .iter()
        .find(|(l, _)| *l == lang)
        .map_or("{}", |(_, raw)| raw)
}

fn table(lang: Lang) -> &'static HashMap<String, String> {
    static TABLES: [OnceLock<HashMap<String, String>>; 15] = [const { OnceLock::new() }; 15];
    TABLES[usize::from(lang.index())]
        .get_or_init(|| serde_json::from_str(source(lang)).unwrap_or_default())
}

/// Raw template for `key` in `lang` only (no fallback).
pub(crate) fn raw(lang: Lang, key: &str) -> Option<&'static str> {
    table(lang).get(key).map(String::as_str)
}

/// Replace `{name}` placeholders; unknown placeholders are left as written.
pub(crate) fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Pure translation: `lang`, then English, then the key.
pub(crate) fn translate(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    match raw(lang, key).or_else(|| raw(Lang::En, key)) {
        Some(template) => fill(template, args),
        None => key.to_string(),
    }
}

/// Subscribe the running component (if any) to language changes.
fn subscribe() {
    if Runtime::try_current().is_some() {
        let _ = LANGUAGE_REVISION.read();
    }
}

pub fn t(key: &str) -> String {
    t_args(key, &[])
}

pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    subscribe();
    translate(language(), key, args)
}

/// Pure plural lookup: `<key>.<category>` in `lang`, then `<key>.other` in `lang`, then English.
pub(crate) fn translate_count(lang: Lang, key: &str, n: usize) -> String {
    let count = n.to_string();
    let args = [("count", count.as_str())];
    let cat = plural_category(lang, n);
    for k in [format!("{key}.{cat}"), format!("{key}.other")] {
        // Some `.one` texts spell out "1" instead of `{count}`; French also picks `.one` for 0,
        // so such a text is only used when the number really is 1.
        let literal_one = |t: &str| !t.contains("{count}") && n != 1;
        if let Some(template) = raw(lang, &k).filter(|t| !(k.ends_with(".one") && literal_one(t))) {
            return fill(template, &args);
        }
    }
    let en = plural_category(Lang::En, n);
    translate(Lang::En, &format!("{key}.{en}"), &args)
}

/// Plural-aware lookup with the language's own rules (`one`, `few`, `many`, `other`, as the
/// locale file provides them); `{count}` is filled.
pub fn t_count(key: &str, n: usize) -> String {
    subscribe();
    translate_count(language(), key, n)
}
