//! "Paylaş": stores the list on the share relays, copies the short link, says what was published.

use std::sync::Mutex;

use dioxus::prelude::*;

use super::REQUEST;
use crate::backend::api;
use crate::backend::error::Coded;
use crate::i18n::t;
use crate::ui::{Banner, BannerKind, Button, ButtonVariant, Dialog, DialogSize, Icon, Spinner};

/// Kept open for the whole run: on Linux the clipboard text lives only as long as its owner.
static BOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

async fn copy(text: String) -> bool {
    tokio::task::spawn_blocking(move || {
        let mut board = BOARD.lock().unwrap_or_else(|e| e.into_inner());
        if board.is_none() {
            *board = arboard::Clipboard::new().ok();
        }
        board.as_mut().is_some_and(|b| b.set_text(text).is_ok())
    })
    .await
    .unwrap_or(false)
}

fn close() {
    *REQUEST.write() = None;
}

#[component]
pub fn ShareDialog() -> Element {
    let mut copied = use_signal(|| false);
    let mut result = use_resource(move || async move {
        let req = REQUEST()?;
        copied.set(false);
        let made = api::share(req.name, req.tracks, req.single).await;
        if let Ok(link) = &made {
            copied.set(copy(link.clone()).await);
        }
        Some(made)
    });
    let request = REQUEST();
    let title = match &request {
        Some(r) if r.single => t("share.title.song"),
        _ => t("share.title.list"),
    };
    let state = result.read().clone().flatten();
    let footer = match &state {
        Some(Err(_)) => rsx! {
            Button { variant: ButtonVariant::Ghost, onclick: move |_| close(), {t("common.close")} }
            Button { icon: "refresh-cw", onclick: move |_| result.restart(), {t("common.retry")} }
        },
        _ => rsx! {
            Button { variant: ButtonVariant::Secondary, onclick: move |_| close(), {t("common.done")} }
        },
    };
    rsx! {
        Dialog {
            open: request.is_some(),
            on_close: move |_| close(),
            title,
            description: request.as_ref().map(|r| r.name.clone()),
            size: DialogSize::Md,
            footer: Some(footer),
            div { class: "flex flex-col gap-4",
                match state {
                    None => rsx! {
                        div { class: "flex items-center gap-3 py-3 text-sm text-muted", role: "status",
                            Spinner { size: 20 }
                            {t("share.working")}
                        }
                    },
                    Some(Ok(link)) => rsx! { LinkBox { link, copied } },
                    Some(Err(e)) => rsx! { Banner { kind: BannerKind::Error, {e.user_message()} } },
                }
                p { class: "flex items-start gap-2 text-sm text-muted",
                    Icon { name: "info", size: 16, class: "mt-0.5 flex-none" }
                    {t("share.privacy")}
                }
            }
        }
    }
}

#[component]
fn LinkBox(link: String, copied: Signal<bool>) -> Element {
    let mut copied = copied;
    let text = link.clone();
    rsx! {
        div { class: "flex flex-col gap-1.5",
            div { class: "flex items-center gap-2",
                input {
                    id: "ow-share-link",
                    r#type: "text",
                    readonly: true,
                    "aria-label": t("share.link_label"),
                    class: "h-10 min-w-0 flex-1 select-all rounded-md border border-line-strong bg-surface-2 px-3 text-sm text-fg focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent-text",
                    value: "{link}",
                    onfocus: move |_| {
                        document::eval("document.getElementById('ow-share-link')?.select()");
                    },
                }
                Button {
                    id: "ow-share-copy",
                    icon: if copied() { "check" } else { "copy" },
                    onclick: move |_| {
                        let text = text.clone();
                        spawn(async move { copied.set(copy(text).await) });
                    },
                    if copied() { {t("share.copied_short")} } else { {t("share.copy")} }
                }
            }
            p { class: "text-sm text-muted", role: "status",
                if copied() { {t("share.copied")} }
            }
        }
    }
}
