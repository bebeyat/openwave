//! First-run welcome card and the three big starter actions.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::features::shell::slots::OPEN_IMPORT;
use crate::features::tracks::state::update_library;
use crate::i18n::{self, t, t_count, Lang};
use crate::routes::Route;
use crate::store::{self, LanguagePref, SettingsChange};
use crate::ui::{toast_error, toast_success, Button, ButtonVariant, Emblem, Icon};

/// Persist "the user has seen the welcome card".
pub fn finish_onboarding() {
    // A failed settings write only means the card may come back next start.
    let _ = store::settings().update(|s| s.apply(SettingsChange::OnboardingDone(true)));
}

fn set_language(lang: Lang) {
    let pref = LanguagePref::from_lang(lang);
    let _ = store::settings().update(|s| s.apply(SettingsChange::Language(pref)));
    i18n::set_language(lang);
}

/// Pick audio files with the native dialog and add them to the library.
pub fn add_local_music() {
    spawn(async move {
        match api::import_audio().await {
            Ok(tracks) if tracks.is_empty() => {}
            Ok(tracks) => {
                update_library(|lib| lib.add_local_files(&tracks));
                toast_success(t_count("home.local_added", tracks.len()));
            }
            Err(e) => toast_error(e.user_message()),
        }
    });
}

fn focus_search() {
    document::eval("window.ow.focusId('ow-global-search')");
}

/// Icon chip of a tile: the same quiet accent wash on all three (no per-tile gradients).
const CHIP_TEALS: &str = "ow-chip-icon";
const CHIP_VIOLET: &str = "ow-chip-icon";
const CHIP_EMBER: &str = "ow-chip-icon";

#[component]
fn ActionCard(
    icon: &'static str,
    chip: &'static str,
    #[props(into)] title: String,
    #[props(into)] body: String,
    onclick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "ow-tile group relative flex min-h-[176px] flex-col items-start gap-3 overflow-hidden rounded-xl border border-line bg-surface p-6 text-left",
            onclick: move |_| onclick.call(()),
            span { class: "flex h-11 w-11 items-center justify-center rounded-lg {chip}",
                Icon { name: icon, size: 22 }
            }
            span { class: "mt-auto ow-display text-xl font-bold text-fg", "{title}" }
            span { class: "text-sm text-muted", "{body}" }
            span { class: "absolute bottom-5 right-5 flex h-8 w-8 translate-x-1 items-center justify-center rounded-full bg-hover text-fg opacity-0 transition-[opacity,transform] duration-(--ow-dur-hover) group-hover:translate-x-0 group-hover:opacity-100",
                Icon { name: "arrow-right", size: 16 }
            }
        }
    }
}

#[component]
pub fn ActionCards() -> Element {
    let nav = navigator();
    rsx! {
        div { class: "grid grid-cols-3 gap-4 max-[900px]:grid-cols-1",
            ActionCard {
                icon: "search",
                chip: CHIP_TEALS,
                title: t("home.action.search"),
                body: t("home.action.search_body"),
                onclick: move |_| {
                    nav.push(Route::Search { q: String::new() });
                    focus_search();
                },
            }
            ActionCard {
                icon: "download",
                chip: CHIP_VIOLET,
                title: t("home.action.import"),
                body: t("home.import_card.body"),
                onclick: move |_| *OPEN_IMPORT.write() = true,
            }
            ActionCard {
                icon: "folder-open",
                chip: CHIP_EMBER,
                title: t("home.action.local"),
                body: t("home.action.local_body"),
                onclick: move |_| add_local_music(),
            }
        }
    }
}

/// Small, quiet language menu for the card corner (every language under its own name).
#[component]
fn LanguageLinks() -> Element {
    let current = i18n::language();
    rsx! {
        label { class: "absolute right-4 top-3 flex items-center gap-1.5 text-xs text-muted",
            Icon { name: "languages", size: 14 }
            select {
                "aria-label": t("common.language"),
                class: "h-9 cursor-pointer rounded-sm bg-transparent px-1 text-xs font-semibold text-fg outline-none hover:underline focus-visible:outline-2 focus-visible:outline-accent-text",
                value: current.code(),
                onchange: move |e| {
                    if let Some(lang) = Lang::from_code(&e.value()) {
                        set_language(lang);
                    }
                },
                for lang in Lang::ALL {
                    option { key: "{lang.code()}", value: lang.code(), lang: lang.code(), selected: lang == current, "{lang.native_name()}" }
                }
            }
        }
    }
}

#[component]
pub fn WelcomeCard(on_skip: EventHandler<()>) -> Element {
    rsx! {
        section {
            class: "ow-enter ow-welcome relative mb-6 overflow-hidden rounded-2xl p-8 max-[900px]:p-6",
            "aria-labelledby": "ow-welcome",
            LanguageLinks {}
            span {
                class: "ow-accent-fill mb-5 flex h-12 w-12 items-center justify-center rounded-xl",
                "aria-hidden": "true",
                Emblem { height: 16 }
            }
            h2 {
                id: "ow-welcome",
                class: "ow-display mb-2 max-w-[820px] text-[clamp(32px,4vw,48px)] font-bold leading-[1.1] tracking-[-0.03em] text-fg",
                {t("home.welcome.title")}
            }
            p { class: "mb-4 max-w-[520px] text-lg text-muted", {t("home.welcome.body")} }
            Button {
                variant: ButtonVariant::Ghost,
                class: "-ml-4",
                onclick: move |_| on_skip.call(()),
                {t("common.skip_for_now")}
            }
        }
    }
}
