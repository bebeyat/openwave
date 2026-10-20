//! Steps 1 and 2 of the import dialog: choose a source, then paste a link.

use dioxus::prelude::*;

use super::brand::{Brand, BrandLogo};
use super::flow::{Ctx, Note};
use super::state::{check_link, Kind, LinkCheck};
use crate::backend::error::Coded;
use crate::i18n::t;
use crate::ui::{Banner, BannerKind, Button, ButtonVariant, Icon, Spinner, TextInput};

/// One logo tile: the brand mark with a short name under it. The longer description stays in
/// the accessible name and the native tooltip.
#[component]
fn SourceButton(
    id: Option<&'static str>,
    brand: Brand,
    name: String,
    help: String,
    chip: Option<String>,
    onclick: EventHandler<()>,
) -> Element {
    let label = match &chip {
        Some(c) => format!("{name}: {help} ({c})"),
        None => format!("{name}: {help}"),
    };
    rsx! {
        button {
            id,
            r#type: "button",
            class: "ow-card-glass ow-tile relative flex aspect-[5/4] w-full flex-col items-center justify-center gap-3 p-4 active:bg-press",
            title: "{help}",
            "aria-label": "{label}",
            onclick: move |_| onclick.call(()),
            BrandLogo { brand }
            span { class: "text-sm font-semibold text-fg", "{name}" }
            if chip.is_some() {
                // A link of this kind is on the clipboard: a small accent badge, named in the label.
                span {
                    class: "ow-chip-icon absolute right-2 top-2 h-6 w-6 rounded-full",
                    "aria-hidden": "true",
                    Icon { name: "clipboard-paste", size: 14 }
                }
            }
        }
    }
}

#[component]
pub fn SourceStep(ctx: Ctx) -> Element {
    let clip = (ctx.clip)();
    let chip = |k: Kind| (clip == Some(k)).then(|| t("import.clipboard_chip"));
    rsx! {
        div { class: "flex flex-col gap-3",
            p { class: "text-sm text-muted", {t("import.source.title")} }
            div { class: "grid grid-cols-2 gap-3 sm:grid-cols-4",
                SourceButton {
                    id: "ow-imp-first-source",
                    brand: Brand::Spotify,
                    name: t("import.source.spotify"),
                    help: t("import.source.spotify.help"),
                    chip: chip(Kind::Spotify),
                    onclick: move |_| ctx.pick(Kind::Spotify),
                }
                SourceButton {
                    brand: Brand::Youtube,
                    name: t("import.source.youtube.short"),
                    help: t("import.source.youtube.help"),
                    chip: chip(Kind::Youtube),
                    onclick: move |_| ctx.pick(Kind::Youtube),
                }
                SourceButton {
                    brand: Brand::Apple,
                    name: t("import.source.apple"),
                    help: t("import.source.apple.help"),
                    chip: chip(Kind::Apple),
                    onclick: move |_| ctx.pick(Kind::Apple),
                }
                SourceButton {
                    brand: Brand::Local,
                    name: t("import.source.files.short"),
                    help: t("import.source.files.help"),
                    chip: None,
                    onclick: move |_| ctx.pick_files(),
                }
            }
        }
    }
}

fn placeholder(kind: Kind) -> String {
    match kind {
        Kind::Spotify => t("import.link.placeholder_spotify"),
        Kind::Youtube => t("import.link.placeholder_youtube"),
        Kind::Apple => t("import.link.placeholder_apple"),
    }
}

fn where_text(kind: Kind) -> String {
    match kind {
        Kind::Spotify => t("import.where.spotify"),
        Kind::Youtube => t("import.where.youtube"),
        Kind::Apple => t("import.where.apple"),
    }
}

fn note_text(note: Note) -> Option<String> {
    match note {
        Note::Found => None,
        Note::Empty => Some(t("import.clipboard_empty")),
        Note::Unavailable => Some(t("import.clipboard_unavailable")),
    }
}

#[component]
pub fn LinkStep(ctx: Ctx) -> Element {
    let kind = (ctx.kind)();
    let input = (ctx.input)();
    let check = check_link(&input);
    let error = match &check {
        LinkCheck::Invalid(e) => Some(e.user_message()),
        _ => None,
    };
    let valid = matches!(check, LinkCheck::Valid(_));
    let note = (ctx.note)();
    let hint = if valid && note == Some(Note::Found) {
        Some(t("import.clipboard_found"))
    } else if valid {
        Some(t("import.link.valid"))
    } else {
        note.and_then(note_text)
    };
    let heading = match kind {
        Kind::Spotify => t("import.source.spotify"),
        Kind::Youtube => t("import.source.youtube"),
        Kind::Apple => t("import.source.apple"),
    };
    rsx! {
        div { class: "flex flex-col gap-4",
            h3 { class: "ow-display text-lg font-bold text-fg", "{heading}" }
            div { class: "flex items-start gap-2",
                TextInput {
                    id: "ow-imp-link",
                    class: "min-w-0 flex-1",
                    label: t("import.link.label"),
                    value: input,
                    placeholder: placeholder(kind),
                    icon: "link",
                    error,
                    hint: if valid { hint.clone() } else { None },
                    oninput: move |v: String| ctx.set_input(v),
                    onsubmit: move |_| if valid { ctx.submit() },
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "clipboard-paste",
                    class: "mt-[27px]",
                    onclick: move |_| ctx.paste(),
                    {t("import.paste")}
                }
            }
            if !valid {
                if let Some(h) = hint {
                    p { class: "-mt-2 text-sm text-muted", "{h}" }
                }
            }
            if let Some(e) = (ctx.link_error)() {
                Banner {
                    kind: BannerKind::Error,
                    action: rsx! {
                        if matches!(e, crate::backend::error::ImportError::Offline | crate::backend::error::ImportError::Unreadable) {
                            Button {
                                variant: ButtonVariant::Secondary,
                                size: crate::ui::ButtonSize::Sm,
                                onclick: move |_| ctx.submit(),
                                {t("common.retry")}
                            }
                        }
                    },
                    {e.user_message()}
                }
            }
            if kind == Kind::Spotify && *super::account::SIGNED_IN.read() {
                super::account::AccountPicker { ctx }
            }
            details { class: "ow-card-glass group px-4 py-2",
                summary { class: "flex min-h-9 cursor-pointer list-none items-center gap-2 text-sm font-semibold text-fg",
                    Icon { name: "circle-help", size: 18, class: "text-muted" }
                    {t("import.where.title")}
                }
                p { class: "mt-2 text-sm text-muted", {where_text(kind)} }
            }
            p { class: "flex items-start gap-2 text-sm text-muted",
                Icon { name: "info", size: 16, class: "mt-0.5 flex-none" }
                {t("import.note_public")}
            }
        }
    }
}

#[component]
pub fn BusyStep(label: String, spinner: bool) -> Element {
    rsx! {
        div { class: "flex flex-col items-center gap-4 py-10 text-center", role: "status",
            if spinner {
                span { class: "ow-chip-icon h-14 w-14 rounded-full", Spinner { size: 28, class: "text-on-accent!" } }
            }
            p { class: "ow-display text-lg font-semibold text-fg", "{label}" }
        }
    }
}
