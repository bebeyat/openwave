//! Modal dialog: focus trap (`window.ow.trapOpen`), Esc to close, backdrop click, and
//! enter/exit animation (240ms; zero in low-power mode / reduced motion).
//!
//! ```ignore
//! Dialog { open: open(), on_close: move |_| open.set(false), title: "Çalma listesi getir",
//!     DialogBody { ... }
//!     footer: rsx! { Button { ... } } }
//! ```
//! Focus returns to the element that opened the dialog. Put `data-autofocus` on the control
//! that should receive focus first (defaults to the first focusable control).

use std::time::Duration;

use dioxus::prelude::*;

use super::button::IconButton;
use super::cls::next_id;
use super::theme::exit_ms;

const DIALOG_MS: u64 = 240;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum DialogSize {
    Sm,
    #[default]
    Md,
}

/// Keeps a conditionally shown overlay mounted while its exit animation runs.
/// Returns `(mounted, closing)`.
pub fn use_presence(open: bool, exit_base_ms: u64) -> (bool, bool) {
    let mut mounted = use_signal(|| open);
    let mut latest_open = use_signal(|| open);
    use_effect(use_reactive!(|open| {
        latest_open.set(open);
        if open {
            mounted.set(true);
        } else if *mounted.peek() {
            spawn(async move {
                tokio::time::sleep(Duration::from_millis(exit_ms(exit_base_ms))).await;
                if !*latest_open.peek() {
                    mounted.set(false);
                }
            });
        }
    }));
    (open || mounted(), !open)
}

#[component]
pub fn Dialog(
    open: bool,
    on_close: EventHandler<()>,
    #[props(into)] title: String,
    #[props(into, default)] description: Option<String>,
    #[props(default)] size: DialogSize,
    /// Backdrop click and Esc close the dialog unless false (e.g. mid-import).
    #[props(default = true)]
    dismissable: bool,
    #[props(default)] footer: Option<Element>,
    children: Element,
) -> Element {
    let (mounted, closing) = use_presence(open, DIALOG_MS);
    if !mounted {
        return rsx! {};
    }
    rsx! {
        DialogShell { title, description, size, dismissable, closing, on_close, footer, {children} }
    }
}

#[component]
fn DialogShell(
    title: String,
    description: Option<String>,
    size: DialogSize,
    dismissable: bool,
    closing: bool,
    on_close: EventHandler<()>,
    footer: Option<Element>,
    children: Element,
) -> Element {
    let id = use_hook(|| next_id("dialog"));
    let title_id = format!("{id}-title");
    let desc_id = format!("{id}-desc");
    let trap_id = id.clone();
    use_effect(move || {
        document::eval(&format!("window.ow.trapOpen('{trap_id}')"));
    });
    let drop_id = id.clone();
    use_drop(move || {
        document::eval(&format!("window.ow.trapClose('{drop_id}')"));
    });

    let width = match size {
        DialogSize::Sm => "max-w-[400px]",
        DialogSize::Md => "max-w-[560px]",
    };
    let (backdrop_anim, panel_anim, events) = if closing {
        ("ow-fade-out", "ow-pop-out", "pointer-events-none")
    } else {
        ("ow-fade-in", "ow-pop-in", "")
    };
    rsx! {
        div { class: "fixed inset-0 z-50 flex items-center justify-center p-6 {events}",
            "data-ow-overlay": "dialog",
            div {
                class: "absolute inset-0 bg-scrim {backdrop_anim}",
                onclick: move |_| if dismissable { on_close.call(()) },
            }
            div {
                id: "{id}",
                role: "dialog",
                tabindex: "-1",
                "aria-modal": "true",
                "aria-labelledby": "{title_id}",
                "aria-describedby": description.as_ref().map(|_| desc_id.clone()),
                class: "ow-glass relative flex max-h-full w-full flex-col overflow-hidden rounded-xl {width} {panel_anim}",
                onkeydown: move |e| {
                    if e.key() == Key::Escape && dismissable {
                        e.stop_propagation();
                        on_close.call(());
                    }
                },
                div { class: "flex items-start gap-4 px-6 pt-5",
                    div { class: "min-w-0 flex-1",
                        h2 { id: "{title_id}", class: "text-2xl font-bold text-fg", "{title}" }
                        if let Some(d) = description.clone() {
                            p { id: "{desc_id}", class: "mt-1 text-sm text-muted", "{d}" }
                        }
                    }
                    if dismissable {
                        IconButton {
                            icon: "x",
                            label: crate::i18n::t("common.close"),
                            shortcut: "Esc",
                            size: super::button::IconButtonSize::Sm,
                            onclick: move |_| on_close.call(()),
                        }
                    }
                }
                div {
                    class: "min-h-0 flex-1 overflow-y-auto px-6 py-4",
                    "data-dialog-body": "true",
                    {children}
                }
                if let Some(f) = footer {
                    div { class: "flex flex-wrap items-center justify-end gap-2 border-t border-line px-6 py-4",
                        {f}
                    }
                }
            }
        }
    }
}
