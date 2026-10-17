//! The one import dialog. Consumes `OPEN_IMPORT` (any entry point sets it) and walks
//! Source -> Link -> Preview -> Result. Nothing is saved before "Kitaplığıma ekle".

use dioxus::prelude::*;

use super::flow::{use_ctx, Ctx, Outcome};
use super::link_step::{BusyStep, LinkStep, SourceStep};
use super::result_steps::{FailedStep, MatchingStep, PreviewStep, ResultStep};
use super::state::{check_link, focus_target, needs_close_confirm, LinkCheck, Step};
use crate::features::shell::slots::OPEN_IMPORT;
use crate::i18n::t;
use crate::player::controller;
use crate::routes::Route;
use crate::ui::{Banner, BannerKind, Button, ButtonVariant, Dialog, DialogSize};

fn close(ctx: Ctx) {
    ctx.cancel_work();
    *OPEN_IMPORT.write() = false;
}

fn request_close(ctx: Ctx) {
    if *ctx.confirming.peek() {
        let mut c = ctx.confirming;
        c.set(false);
    } else if needs_close_confirm(*ctx.step.peek()) {
        let mut c = ctx.confirming;
        c.set(true);
    } else {
        close(ctx);
    }
}

#[component]
pub fn ImportDialog() -> Element {
    let ctx = use_ctx();
    let open = OPEN_IMPORT();
    use_effect(move || {
        if OPEN_IMPORT() {
            ctx.reset();
            ctx.peek_clipboard();
        }
    });
    // Keep keyboard focus on the natural control of each step.
    use_effect(move || {
        let id = if (ctx.confirming)() {
            "ow-imp-primary"
        } else {
            focus_target((ctx.step)())
        };
        document::eval(&format!("window.ow.focusId('{id}')"));
    });
    rsx! {
        Dialog {
            open,
            on_close: move |_| request_close(ctx),
            title: t("import.title"),
            size: DialogSize::Md,
            footer: footer(ctx),
            if (ctx.confirming)() {
                ConfirmClose { ctx }
            } else {
                Body { ctx }
            }
        }
    }
}

#[component]
fn ConfirmClose(ctx: Ctx) -> Element {
    rsx! {
        Banner { kind: BannerKind::Warning, title: t("import.confirm_close.title"),
            {t("import.confirm_close.body")}
        }
    }
}

#[component]
fn Body(ctx: Ctx) -> Element {
    match (ctx.step)() {
        Step::Source => rsx! { SourceStep { ctx } },
        Step::Link => rsx! { LinkStep { ctx } },
        Step::Reading => rsx! { BusyStep { label: t("import.reading"), spinner: true } },
        Step::Adding => rsx! { BusyStep { label: t("import.adding_files"), spinner: true } },
        Step::Preview => rsx! { PreviewStep { ctx } },
        Step::Matching => rsx! { MatchingStep { ctx } },
        Step::Result => match (ctx.outcome)() {
            Some(outcome) => rsx! { ResultStep { outcome } },
            None => rsx! {},
        },
        Step::Failed => {
            let message = (ctx.failure)().map(|f| f.message).unwrap_or_default();
            rsx! { FailedStep { message } }
        }
    }
}

fn footer(ctx: Ctx) -> Option<Element> {
    if (ctx.confirming)() {
        return Some(rsx! {
            Button {
                id: "ow-imp-primary",
                variant: ButtonVariant::Secondary,
                onclick: move |_| {
                    let mut c = ctx.confirming;
                    c.set(false);
                },
                {t("import.confirm_close.stay")}
            }
            Button {
                variant: ButtonVariant::Danger,
                onclick: move |_| close(ctx),
                {t("import.confirm_close.leave")}
            }
        });
    }
    match (ctx.step)() {
        Step::Source => None,
        Step::Link => Some(link_footer(ctx)),
        Step::Reading | Step::Matching | Step::Adding => Some(rsx! {
            Button {
                id: "ow-imp-cancel",
                variant: ButtonVariant::Secondary,
                onclick: move |_| {
                    if *ctx.step.peek() == Step::Matching {
                        request_close(ctx);
                    } else if *ctx.step.peek() == Step::Reading {
                        ctx.back();
                    } else {
                        close(ctx);
                    }
                },
                {t("import.cancel")}
            }
        }),
        Step::Preview => Some(rsx! {
            Button { variant: ButtonVariant::Ghost, onclick: move |_| close(ctx), {t("import.cancel")} }
            Button { id: "ow-imp-confirm", icon: "list-plus", onclick: move |_| ctx.confirm(), {t("import.confirm")} }
        }),
        Step::Result => (ctx.outcome)().map(|o| result_footer(ctx, o)),
        Step::Failed => Some(failed_footer(ctx)),
    }
}

fn link_footer(ctx: Ctx) -> Element {
    let valid = matches!(check_link(&(ctx.input)()), LinkCheck::Valid(_));
    rsx! {
        Button { variant: ButtonVariant::Ghost, class: "mr-auto", icon: "arrow-left", onclick: move |_| ctx.back(), {t("common.back")} }
        Button { id: "ow-imp-continue", disabled: !valid, onclick: move |_| ctx.submit(), {t("common.continue")} }
    }
}

fn failed_footer(ctx: Ctx) -> Element {
    let retry = (ctx.failure)().is_some_and(|f| f.retry);
    rsx! {
        Button { variant: ButtonVariant::Ghost, onclick: move |_| close(ctx), {t("common.close")} }
        Button {
            id: "ow-imp-primary",
            variant: if retry { ButtonVariant::Primary } else { ButtonVariant::Secondary },
            onclick: move |_| if retry { ctx.retry() } else { ctx.back_from_failure() },
            if retry { {t("common.retry")} } else { {t("common.back")} }
        }
    }
}

fn result_footer(ctx: Ctx, outcome: Outcome) -> Element {
    let nav = navigator();
    let playlist_id = outcome.playlist_id.clone();
    let tracks = outcome.tracks.clone();
    let open_target = playlist_id.clone();
    rsx! {
        Button { variant: ButtonVariant::Ghost, onclick: move |_| close(ctx), {t("common.done")} }
        Button {
            variant: ButtonVariant::Secondary,
            icon: "list-music",
            onclick: move |_| {
                close(ctx);
                let route = match &open_target {
                    Some(id) => Route::Playlist { id: id.clone() },
                    None => Route::Library { tab: "local".into() },
                };
                nav.push(route);
            },
            if playlist_id.is_some() { {t("import.result.open_playlist")} } else { {t("import.result.open_local")} }
        }
        Button {
            id: "ow-imp-primary",
            icon: "play",
            onclick: move |_| {
                controller::play_now(&tracks, 0);
                close(ctx);
            },
            {t("import.result.play_now")}
        }
    }
}
