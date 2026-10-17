//! Hakkında: version, audio engine (yt-dlp) status and update check, licences, repository.

use dioxus::prelude::*;

use super::rows::{Card, Row, SectionTitle};
use crate::backend::api;
use crate::backend::error::Coded;
use crate::features::onboarding::engine::{self, percent, Engine, ENGINE};
use crate::i18n::{t, t_args};
use crate::ui::{Button, ButtonVariant, ProgressBar, Spinner};

/// Placeholder until the owner publishes the repository (nothing is pushed without their go-ahead).
pub const REPO_URL: &str = "https://github.com/bebeyat/openwave";
pub const NOTICES_URL: &str =
    "https://github.com/bebeyat/openwave/blob/main/THIRD_PARTY_NOTICES.md";

pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn open_link(url: &str) {
    let _ = open::that(url);
}

#[derive(Clone, PartialEq)]
enum Update {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed(String),
}

#[component]
fn EngineRow() -> Element {
    use_hook(engine::refresh);
    let update = use_signal(|| Update::Idle);
    let state = ENGINE();
    let (summary, controls) = match state {
        Engine::Checking => (
            rsx! { Spinner { size: 18, label: t("engine.checking") } },
            rsx! {},
        ),
        Engine::Ready { version, managed } => {
            let source = if managed {
                t("engine.source.data_dir")
            } else {
                t("engine.source.path")
            };
            (
                rsx! { span { class: "text-sm text-fg", {t_args("engine.version", &[("version", &version)])} " · " "{source}" } },
                update_controls(update),
            )
        }
        Engine::Missing => (
            rsx! { span { class: "text-sm text-fg", {t("engine.missing")} } },
            install_controls(false),
        ),
        Engine::Installing {
            downloaded, total, ..
        } => (
            rsx! {
                div { class: "w-56",
                    ProgressBar { value: percent(downloaded, total), label: t("engine.preparing") }
                }
            },
            rsx! {},
        ),
        Engine::Failed { message } => (
            rsx! { span { class: "text-sm text-danger-text", "{message}" } },
            install_controls(true),
        ),
    };
    let note = match update() {
        Update::Idle => None,
        Update::Checking => Some(t("engine.checking")),
        Update::UpToDate => Some(t("engine.up_to_date")),
        Update::Available(v) => Some(t_args("engine.update_available", &[("version", &v)])),
        Update::Failed(m) => Some(m),
    };
    rsx! {
        Row { label: t("settings.about.engine"), help: t("settings.about.engine.help"), stack: true,
            div { class: "flex flex-wrap items-center gap-3",
                {summary}
                {controls}
                if let Some(n) = note {
                    span { class: "text-sm text-muted", role: "status", "{n}" }
                }
                if matches!(update(), Update::Available(_)) {
                    Button { icon: "download", onclick: move |_| engine::install(), {t("engine.update")} }
                }
            }
        }
    }
}

fn install_controls(retry: bool) -> Element {
    rsx! {
        Button {
            icon: "download",
            onclick: move |_| engine::install(),
            if retry { {t("engine.retry")} } else { {t("engine.install")} }
        }
        Button {
            variant: ButtonVariant::Secondary,
            icon: "folder-open",
            onclick: move |_| engine::pick_manual(),
            {t("engine.pick_manual")}
        }
    }
}

fn update_controls(mut update: Signal<Update>) -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Secondary,
            icon: "refresh-cw",
            loading: update() == Update::Checking,
            onclick: move |_| {
                update.set(Update::Checking);
                spawn(async move {
                    update.set(match api::ytdlp_check_update().await {
                        Ok(c) if c.update_available => Update::Available(c.latest),
                        Ok(_) => Update::UpToDate,
                        Err(e) => Update::Failed(e.user_message()),
                    });
                });
            },
            {t("engine.check_update")}
        }
    }
}

#[component]
pub fn About() -> Element {
    rsx! {
        SectionTitle { title: t("settings.section.about"), intro: t("settings.about.tagline"), icon: "info" }
        Card {
            Row { label: "OpenWave", help: t_args("settings.about.version", &[("version", app_version())]),
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "external-link",
                    onclick: move |_| open_link(REPO_URL),
                    {t("settings.about.github")}
                }
            }
            EngineRow {}
            Row { label: t("settings.about.licenses"), help: t("settings.about.licenses.help"),
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "external-link",
                    onclick: move |_| open_link(NOTICES_URL),
                    {t("settings.about.licenses.button")}
                }
            }
        }
        p { class: "mt-4 text-sm text-muted", {t("settings.about.tos_note")} }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comes_from_cargo() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
        assert!(!app_version().is_empty());
    }

    #[test]
    fn links_are_https() {
        assert!(REPO_URL.starts_with("https://") && NOTICES_URL.starts_with(REPO_URL));
    }
}
