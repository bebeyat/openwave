//! Gizlilik: the plain statement, the complete list of network destinations (mirrors
//! docs/PRIVACY.md), a shortcut to the data folder and "delete all my data".

use dioxus::prelude::*;

use super::rows::{Card, Row, SectionTitle};
use super::wipe::{confirmation_matches, quiesce, wipe_own_data, WipeReport, CONFIRM_WORD};
use crate::backend::api;
use crate::i18n::{t, t_args};
use crate::ui::{Banner, BannerKind, Button, ButtonVariant, Dialog, DialogSize, Icon, TextInput};

/// (destination shown as-is, i18n key of "when and what is sent"). Keep in sync with PRIVACY.md.
pub const NETWORK: [(&str, &str); 9] = [
    ("music.youtube.com", "settings.privacy.row.catalog"),
    (
        "www.youtube.com (visitor id)",
        "settings.privacy.row.visitor",
    ),
    ("www.youtube.com", "settings.privacy.row.player"),
    ("*.googlevideo.com", "settings.privacy.row.audio"),
    (
        "i.ytimg.com · *.ggpht.com · *.googleusercontent.com",
        "settings.privacy.row.images",
    ),
    ("lrclib.net", "settings.privacy.row.lyrics"),
    ("open.spotify.com", "settings.privacy.row.spotify"),
    (
        "github.com · *.githubusercontent.com",
        "settings.privacy.row.github",
    ),
    ("yt-dlp → www.youtube.com", "settings.privacy.row.ytdlp"),
];

/// Close the app (after a wipe the in-memory copies must not be written back).
fn quit() {
    std::process::exit(0);
}

fn open_data_folder() {
    let _ = open::that(api::data_dir());
}

#[component]
pub fn Privacy() -> Element {
    let mut wipe_open = use_signal(|| false);
    rsx! {
        SectionTitle { title: t("settings.section.privacy"), icon: "shield-check" }
        Banner { kind: BannerKind::Success, title: t("settings.privacy.headline"),
            p { {t("settings.privacy.body")} }
            p { class: "mt-2", {t("settings.privacy.except")} }
        }
        h3 { class: "mb-2 mt-6 text-base font-semibold text-fg", {t("settings.privacy.list_title")} }
        Card {
            for (host , key) in NETWORK {
                div { key: "{host}", class: "border-b border-line py-3",
                    div { class: "flex items-center gap-2 text-sm font-semibold text-fg",
                        Icon { name: "globe", size: 16, class: "flex-none text-muted" }
                        span { class: "min-w-0 break-words", "{host}" }
                    }
                    p { class: "mt-0.5 pl-6 text-sm text-muted", {t(key)} }
                }
            }
        }
        p { class: "mt-3 text-sm text-muted", {t("settings.privacy.more")} }
        h3 { class: "mb-2 mt-6 text-base font-semibold text-fg", {t("settings.privacy.data_title")} }
        Card {
            Row { label: t("settings.privacy.show_data"), help: t("settings.privacy.show_data.help"),
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "folder-open",
                    onclick: move |_| open_data_folder(),
                    {t("settings.privacy.show_data.button")}
                }
            }
            Row { label: t("settings.privacy.wipe"), help: t("settings.privacy.wipe.help"),
                Button {
                    variant: ButtonVariant::Danger,
                    icon: "trash-2",
                    onclick: move |_| wipe_open.set(true),
                    {t("settings.privacy.wipe.button")}
                }
            }
        }
        WipeDialog { open: wipe_open(), on_close: move |_| wipe_open.set(false) }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Wiped {
    No,
    Done,
    /// The folder is not clearly OpenWave's, so nothing was deleted.
    Refused,
}

#[component]
fn WipeDialog(open: bool, on_close: EventHandler<()>) -> Element {
    let mut typed = use_signal(String::new);
    let mut state = use_signal(|| Wiped::No);
    let mut outcome = use_signal(WipeReport::default);
    let dir = api::data_dir().display().to_string();
    let ok = confirmation_matches(&typed());
    let done = state() == Wiped::Done;
    let footer = if done {
        rsx! {
            Button { onclick: move |_| quit(), {t("settings.privacy.wipe.quit")} }
        }
    } else {
        rsx! {
            Button { variant: ButtonVariant::Ghost, onclick: move |_| on_close.call(()), {t("common.cancel")} }
            Button {
                variant: ButtonVariant::Danger,
                disabled: !ok,
                onclick: move |_| {
                    // Nothing may write the files again once they are gone.
                    quiesce();
                    match wipe_own_data(&api::data_dir()) {
                        Ok(report) => {
                            outcome.set(report);
                            state.set(Wiped::Done);
                        }
                        Err(_) => state.set(Wiped::Refused),
                    }
                },
                {t("settings.privacy.wipe.confirm")}
            }
        }
    };
    rsx! {
        Dialog {
            open,
            dismissable: !done,
            on_close: move |_| {
                typed.set(String::new());
                on_close.call(());
            },
            title: t("settings.privacy.wipe.title"),
            size: DialogSize::Sm,
            footer: Some(footer),
            if done {
                {wipe_result(&outcome())}
            } else {
                div { class: "flex flex-col gap-4",
                    p { class: "text-sm text-fg", {t_args("settings.privacy.wipe.body", &[("path", &dir)])} }
                    TextInput {
                        label: t_args("settings.privacy.wipe.type", &[("word", CONFIRM_WORD)]),
                        value: typed(),
                        oninput: move |v: String| typed.set(v),
                        error: (state() == Wiped::Refused).then(|| t("settings.privacy.wipe.refused")),
                    }
                }
            }
        }
    }
}

/// Honest result: how many files went, and which ones stay because another program uses them.
fn wipe_result(report: &WipeReport) -> Element {
    if report.locked.is_empty() {
        return rsx! {
            Banner { kind: BannerKind::Success, {t("settings.privacy.wipe.done")} }
        };
    }
    let text = t_args(
        "settings.privacy.wipe.partial",
        &[
            ("deleted", &report.deleted.to_string()),
            ("locked", &report.locked.len().to_string()),
        ],
    );
    let names = report.locked.join(", ");
    rsx! {
        Banner { kind: BannerKind::Warning,
            p { "{text}" }
            p { class: "mt-1 break-words text-xs text-muted", "{names}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NETWORK;

    #[test]
    fn every_network_row_has_text_in_both_languages() {
        for (name, raw) in [
            ("tr", include_str!("../../../assets/locales/tr.json")),
            ("en", include_str!("../../../assets/locales/en.json")),
        ] {
            let map: serde_json::Value = serde_json::from_str(raw).unwrap();
            for (host, key) in NETWORK {
                assert!(map[key].is_string(), "{name}: {key} ({host}) is missing");
            }
        }
    }

    #[test]
    fn page_lists_the_start_up_and_ahead_of_time_requests() {
        let hosts: Vec<_> = NETWORK.iter().map(|(h, _)| *h).collect();
        assert!(hosts.iter().any(|h| h.contains("visitor id")));
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/locales/en.json")).unwrap();
        let player = en["settings.privacy.row.player"].as_str().unwrap();
        assert!(player.contains("ahead of time") && player.contains("never audio"));
        assert!(en["settings.privacy.row.images"]
            .as_str()
            .unwrap()
            .contains("downloads a small copy"));
    }
}
