//! The calm startup banner about the audio engine. Shown only while yt-dlp is missing,
//! being installed, or failed; hidden when ready. It never downloads on its own.

use dioxus::prelude::*;

use super::engine::{self, percent, Engine, BANNER_DISMISSED, ENGINE};
use crate::backend::ytdlp::Phase;
use crate::i18n::{t, t_args};
use crate::ui::{BannerKind, Button, ButtonSize, ButtonVariant, Icon, ProgressBar};

fn phase_text(phase: Phase, pct: Option<f64>) -> String {
    match phase {
        Phase::Downloading => match pct {
            Some(p) => t_args("engine.downloading", &[("percent", &format!("{p:.0}"))]),
            None => t("common.loading"),
        },
        Phase::Verifying => t("engine.verifying"),
        Phase::Installing | Phase::Done => t("engine.installing"),
    }
}

#[component]
fn Actions(retry: bool) -> Element {
    rsx! {
        div { class: "mt-3 flex flex-wrap items-center gap-2",
            Button {
                size: ButtonSize::Sm,
                icon: "download",
                onclick: move |_| engine::install(),
                if retry {
                    {t("engine.retry")}
                } else {
                    {t("engine.install")}
                }
            }
            Button {
                size: ButtonSize::Sm,
                variant: ButtonVariant::Secondary,
                icon: "folder-open",
                onclick: move |_| engine::pick_manual(),
                {t("engine.pick_manual")}
            }
            if !retry {
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| *BANNER_DISMISSED.write() = true,
                    {t("engine.consent.decline")}
                }
            }
        }
    }
}

/// The banner card: a glass card with a gradient icon chip, a title and the message below it.
#[component]
fn EngineCard(kind: BannerKind, #[props(into)] title: String, children: Element) -> Element {
    let (icon, hue, role) = match kind {
        BannerKind::Error => ("circle-alert", "--ca:#fb7185;--cb:#f43f5e", "alert"),
        _ => (
            "download",
            "--ca:var(--ow-accent);--cb:var(--ow-accent-2)",
            "status",
        ),
    };
    rsx! {
        div { role, class: "ow-card-glass ow-card-deep ow-corner-glow flex items-start gap-4 p-4",
            span { class: "ow-chip-hue h-11 w-11 rounded-lg", style: "{hue}", "aria-hidden": "true",
                Icon { name: icon, size: 22 }
            }
            div { class: "min-w-0 flex-1",
                div { class: "ow-display text-base font-bold text-fg", "{title}" }
                div { class: "mt-0.5 text-sm text-fg/90", {children} }
            }
        }
    }
}

#[component]
pub fn EngineBanner() -> Element {
    use_hook(engine::refresh);
    if BANNER_DISMISSED() {
        return rsx! {};
    }
    let body = match ENGINE() {
        Engine::Checking | Engine::Ready { .. } => return rsx! {},
        Engine::Missing => rsx! {
            EngineCard { kind: BannerKind::Info, title: t("engine.banner.ask_title"),
                {t("engine.banner.body")}
                Actions { retry: false }
            }
        },
        Engine::Installing {
            phase,
            downloaded,
            total,
        } => {
            let pct = percent(downloaded, total);
            rsx! {
                EngineCard { kind: BannerKind::Info, title: t("engine.preparing"),
                    div { class: "flex flex-col gap-2",
                        span { role: "status", {phase_text(phase, pct)} }
                        ProgressBar { class: "h-2", value: pct, label: t("engine.preparing") }
                    }
                }
            }
        }
        Engine::Failed { message } => rsx! {
            EngineCard { kind: BannerKind::Error, title: t("engine.banner.failed_title"),
                "{message}"
                Actions { retry: true }
            }
        },
    };
    rsx! {
        div { class: "ow-page flex-none pb-3 pt-1", {body} }
    }
}
