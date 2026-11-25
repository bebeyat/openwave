//! "Birlikte dinle": the player bar button and the dialog that starts, joins and ends a session.

use dioxus::prelude::*;

use super::{join, leave, start_host, Session, DIALOG_OPEN, SESSION};
use crate::backend::listen;
use crate::features::share::{copy, LinkBox};
use crate::i18n::t;
use crate::ui::{
    Button, ButtonVariant, Dialog, DialogSize, IconButton, IconButtonSize, Spinner, TextInput,
    TooltipAlign,
};

fn close() {
    *DIALOG_OPEN.write() = false;
}

/// The pressed button is gone once a session starts: keep the focus (and Esc) in the dialog.
fn keep_focus() {
    document::eval("document.querySelector('[role=dialog]')?.focus()");
}

#[component]
pub fn ListenButton() -> Element {
    let on = SESSION.read().is_some();
    rsx! {
        IconButton {
            id: "ow-listen-btn",
            icon: "users",
            label: t("listen.title"),
            size: IconButtonSize::Sm,
            pressed: Some(on),
            indicator: on,
            haspopup: "dialog",
            tooltip_align: TooltipAlign::End,
            onclick: move |_| *DIALOG_OPEN.write() = true,
        }
    }
}

#[component]
pub fn ListenDialog() -> Element {
    let session = SESSION();
    let footer = match &session {
        Some(s) => rsx! {
            Button { variant: ButtonVariant::Ghost, onclick: move |_| close(), {t("common.close")} }
            Button {
                variant: ButtonVariant::Danger,
                onclick: move |_| leave(),
                if s.host { {t("listen.end")} } else { {t("listen.leave")} }
            }
        },
        None => rsx! {
            Button { variant: ButtonVariant::Secondary, onclick: move |_| close(), {t("common.close")} }
        },
    };
    rsx! {
        Dialog {
            open: DIALOG_OPEN(),
            on_close: move |_| close(),
            title: t("listen.title"),
            size: DialogSize::Md,
            footer: Some(footer),
            match session {
                None => rsx! { Start {} },
                Some(s) if s.host => rsx! { Hosting { code: s.code } },
                Some(s) => rsx! { Guest { session: s } },
            }
        }
    }
}

#[component]
fn Start() -> Element {
    let mut input = use_signal(String::new);
    let mut tried = use_signal(|| false);
    let code = listen::parse_code(&input());
    let error = (tried() && code.is_none()).then(|| t("listen.bad_code"));
    let mut submit = move || match listen::parse_code(&input.peek()) {
        Some(code) => join(code),
        None => tried.set(true),
    };
    rsx! {
        div { class: "flex flex-col gap-5",
            Button { icon: "users", class: "self-start", onclick: move |_| start_host(), {t("listen.start")} }
            div { class: "flex items-start gap-2",
                TextInput {
                    id: "ow-listen-code",
                    class: "min-w-0 flex-1",
                    label: t("listen.code_label"),
                    value: input(),
                    icon: "link",
                    error,
                    oninput: move |v: String| {
                        input.set(v);
                        tried.set(false);
                    },
                    onsubmit: move |_| submit(),
                }
                Button { variant: ButtonVariant::Secondary, class: "mt-7", onclick: move |_| submit(), {t("listen.join")} }
            }
        }
    }
}

#[component]
fn Hosting(code: String) -> Element {
    let link = listen::link(&code);
    let mut copied = use_signal(|| false);
    let text = link.clone();
    use_hook(move || {
        keep_focus();
        spawn(async move { copied.set(copy(text).await) })
    });
    rsx! {
        div { class: "flex flex-col gap-3",
            p { class: "text-sm text-muted", {t("listen.hosting")} }
            LinkBox { link, copied }
        }
    }
}

#[component]
fn Guest(session: Session) -> Element {
    use_hook(keep_focus);
    rsx! {
        div { class: "flex items-center gap-3 py-2 text-sm", role: "status",
            if session.live {
                span { class: "h-2 w-2 rounded-full bg-accent-text", "aria-hidden": "true" }
                {t("listen.following")}
            } else {
                Spinner { size: 20 }
                span { class: "text-muted", {t("listen.waiting")} }
            }
        }
    }
}
