//! Toasts: `ToastHost {}` once in the app root, `toast("text")` from anywhere.
//!
//! ```ignore
//! toast("Kitaplığına eklendi");            // info
//! toast_success("120 şarkı eklendi");
//! toast_error("Bağlantı kurulamadı. Tekrar dene.");
//! ```
//! Errors use `role="alert"` and stay until dismissed; the others are polite `status` messages that
//! fade after a few seconds. The timer pauses while the pointer or keyboard focus is on a toast.
//! Only the per-item role announces (the host is not a live region), so each toast is read once.
//! Call from event handlers, effects or futures (anything inside the Dioxus runtime).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dioxus::prelude::*;

use super::icon::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Error,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ToastItem {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
}

static TOASTS: GlobalSignal<Vec<ToastItem>> = Signal::global(Vec::new);
static NEXT_TOAST: AtomicU64 = AtomicU64::new(1);
const MAX_VISIBLE: usize = 4;

pub fn toast_kind(kind: ToastKind, text: impl Into<String>) {
    let item = ToastItem {
        id: NEXT_TOAST.fetch_add(1, Ordering::Relaxed),
        kind,
        text: text.into(),
    };
    TOASTS.with_mut(|list| {
        list.push(item);
        let excess = list.len().saturating_sub(MAX_VISIBLE);
        list.drain(..excess);
    });
}

pub fn toast(text: impl Into<String>) {
    toast_kind(ToastKind::Info, text);
}

pub fn toast_success(text: impl Into<String>) {
    toast_kind(ToastKind::Success, text);
}

pub fn toast_error(text: impl Into<String>) {
    toast_kind(ToastKind::Error, text);
}

fn dismiss(id: u64) {
    TOASTS.with_mut(|list| list.retain(|t| t.id != id));
}

/// Remaining lifetime of a toast; `None` for errors, which stay until dismissed.
fn lifetime(kind: ToastKind) -> Option<Duration> {
    match kind {
        ToastKind::Error => None,
        _ => Some(Duration::from_secs(5)),
    }
}

const TICK: Duration = Duration::from_millis(100);

#[component]
fn ToastView(item: ToastItem) -> Element {
    let id = item.id;
    let kind = item.kind;
    let mut hovered = use_signal(|| false);
    let mut focused = use_signal(|| false);
    use_future(move || async move {
        let Some(mut left) = lifetime(kind) else {
            return;
        };
        while !left.is_zero() {
            tokio::time::sleep(TICK).await;
            if !hovered() && !focused() {
                left = left.saturating_sub(TICK);
            }
        }
        dismiss(id);
    });
    let (icon, tone, role) = match item.kind {
        ToastKind::Info => ("info", "text-info", "status"),
        ToastKind::Success => ("circle-check", "text-success", "status"),
        ToastKind::Error => ("circle-alert", "text-danger-text", "alert"),
    };
    rsx! {
        div {
            role,
            onmouseenter: move |_| hovered.set(true),
            onmouseleave: move |_| hovered.set(false),
            onfocusin: move |_| focused.set(true),
            onfocusout: move |_| focused.set(false),
            class: "ow-toast-in pointer-events-auto ow-glass flex w-[360px] max-w-full items-start gap-3 rounded-lg py-3 pl-4 pr-2",
            Icon { name: icon, size: 20, class: "mt-0.5 {tone}" }
            p { class: "min-w-0 flex-1 py-0.5 text-sm text-fg", "{item.text}" }
            button {
                r#type: "button",
                "aria-label": crate::i18n::t("common.close"),
                class: "flex h-9 w-9 flex-none items-center justify-center rounded-full text-muted hover:bg-hover hover:text-fg",
                onclick: move |_| dismiss(id),
                Icon { name: "x", size: 16 }
            }
        }
    }
}

#[component]
pub fn ToastHost() -> Element {
    let items = TOASTS();
    rsx! {
        div {
            class: "pointer-events-none fixed bottom-[112px] right-4 z-[70] flex flex-col items-end gap-2",
            for item in items {
                ToastView { key: "{item.id}", item }
            }
        }
    }
}
