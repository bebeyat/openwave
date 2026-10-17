//! Steps 3 and 4 of the import dialog: preview, matching progress, result and failure.

use dioxus::prelude::*;

use super::flow::{Ctx, Outcome};
use super::state::Kind;
use crate::backend::error::{Coded, ImportError};
use crate::i18n::{t, t_args, t_count};
use crate::ui::{Banner, BannerKind, Cover, Icon, ProgressBar, TextInput};

fn truncation_text(w: &ImportError) -> String {
    w.user_message()
}

#[component]
pub fn PreviewStep(ctx: Ctx) -> Element {
    let Some(prepared) = (ctx.prepared)() else {
        return rsx! {};
    };
    let preview = prepared.preview;
    let cover = preview.cover.as_ref().map(|c| c.url.to_string());
    rsx! {
        div { class: "flex flex-col gap-4",
            div { class: "ow-card-glass ow-corner-glow flex items-center gap-5 p-5",
                span { class: "flex-none rounded-md shadow-[0_18px_44px_-12px_var(--ow-accent)]",
                    Cover { src: cover, alt: "", size: 120 }
                }
                div { class: "min-w-0 flex-1",
                    TextInput {
                        id: "ow-imp-name",
                        label: t("import.name_label"),
                        value: (ctx.name)(),
                        oninput: move |v: String| {
                            let mut n = ctx.name;
                            n.set(v);
                        },
                        onsubmit: move |_| ctx.confirm(),
                    }
                    p { class: "mt-1.5 text-sm text-muted", {t_count("import.preview.songs", preview.count)} }
                }
            }
            if let Some(w) = preview.warning.as_ref() {
                Banner { kind: BannerKind::Warning, {truncation_text(w)} }
            }
            p { class: "text-sm text-muted", {t("import.preview.nothing_saved")} }
        }
    }
}

#[component]
pub fn MatchingStep(ctx: Ctx) -> Element {
    let (done, total) = (ctx.progress)();
    let kind = (ctx.kind)();
    let pct = (total > 0).then(|| done as f64 * 100.0 / total as f64);
    let label = if kind == Kind::Youtube {
        t("import.saving")
    } else {
        t_args(
            "import.progress",
            &[("done", &done.to_string()), ("total", &total.to_string())],
        )
    };
    rsx! {
        div { class: "ow-card-glass flex flex-col gap-4 px-6 py-7",
            div { class: "flex items-center gap-3",
                span { class: "ow-chip-icon h-11 w-11 rounded-full",
                    Icon { name: "list-music", size: 22 }
                }
                p { class: "ow-display text-lg font-bold text-fg", role: "status", "aria-live": "polite", "{label}" }
            }
            ProgressBar {
                class: "h-2.5",
                value: if kind == Kind::Youtube { None } else { pct },
                label: t("import.progress.aria"),
            }
        }
    }
}

#[component]
fn MissingList(items: Vec<(String, String)>) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "flex flex-col gap-2",
            button {
                r#type: "button",
                "aria-expanded": "{open()}",
                class: "inline-flex h-9 w-fit items-center gap-2 rounded-md px-2 text-sm font-semibold text-accent-text hover:bg-hover",
                onclick: move |_| open.set(!open()),
                Icon { name: if open() { "chevron-up" } else { "chevron-down" }, size: 16 }
                {t("import.result.which")}
            }
            if open() {
                ul { class: "ow-card-glass max-h-48 overflow-y-auto rounded-lg",
                    for (i , (title , artists)) in items.into_iter().enumerate() {
                        li { key: "{i}", class: "flex gap-2 border-b border-line px-3 py-2 text-sm last:border-b-0",
                            span { class: "min-w-0 truncate font-semibold text-fg", "{title}" }
                            if !artists.is_empty() {
                                span { class: "min-w-0 flex-none truncate text-muted", "{artists}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn headline(o: &Outcome) -> String {
    let added = o.tracks.len().to_string();
    if o.playlist_id.is_none() {
        return t_args("import.result.files_added", &[("count", &added)]);
    }
    if o.missing.is_empty() {
        t_args("import.result.added_all", &[("added", &added)])
    } else {
        t_args(
            "import.result.added_missing",
            &[("added", &added), ("missing", &o.missing.len().to_string())],
        )
    }
}

#[component]
pub fn ResultStep(outcome: Outcome) -> Element {
    let text = headline(&outcome);
    let items: Vec<(String, String)> = outcome
        .missing
        .iter()
        .map(|m| (m.title.clone(), m.artists.clone()))
        .collect();
    rsx! {
        div { class: "flex flex-col gap-4",
            div { class: "flex items-center gap-3", role: "status",
                span {
                    class: "ow-chip-hue h-12 w-12 rounded-full",
                    style: "--ca:#4ade80;--cb:#2dd4bf",
                    Icon { name: "circle-check", size: 26 }
                }
                p { class: "ow-display text-lg font-bold text-fg", "{text}" }
            }
            if let Some(w) = outcome.warning.as_ref() {
                Banner { kind: BannerKind::Warning, {truncation_text(w)} }
            }
            if !items.is_empty() {
                MissingList { items }
            }
        }
    }
}

#[component]
pub fn FailedStep(message: String) -> Element {
    rsx! {
        div { class: "flex flex-col gap-3",
            Banner { kind: BannerKind::Error, title: t("import.failed.title"), "{message}" }
        }
    }
}
