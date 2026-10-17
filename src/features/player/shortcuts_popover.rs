//! The "?" popover with the list of keyboard shortcuts (data in `shell::shortcut_list`).

use dioxus::prelude::*;

use super::state::SHORTCUTS_OPEN;
use crate::features::shell::shortcut_list::SHORTCUTS;
use crate::i18n::t;
use crate::ui::{IconButton, IconButtonSize, Kbd};

const POPOVER_ID: &str = "ow-shortcuts-pop";
const RETURN_FOCUS_JS: &str = "setTimeout(function(){var a=document.activeElement;if(!a||a===document.body){var e=document.getElementById('ow-shortcuts-btn');if(e)e.focus();}},60);";

#[component]
pub fn ShortcutsPopover() -> Element {
    if !*SHORTCUTS_OPEN.read() {
        return rsx! {};
    }
    rsx! { Panel {} }
}

#[component]
fn Panel() -> Element {
    use_effect(|| {
        document::eval(&format!("window.ow.focusId('{POPOVER_ID}')"));
    });
    use_drop(|| {
        document::eval(RETURN_FOCUS_JS);
    });
    let close = move |_| *SHORTCUTS_OPEN.write() = false;
    rsx! {
        div { class: "fixed inset-0 z-40", onclick: close }
        div {
            id: POPOVER_ID,
            role: "dialog",
            tabindex: "-1",
            "aria-label": t("shortcuts.title"),
            class: "ow-menu-in ow-glass fixed bottom-[104px] right-4 z-50 w-[360px] max-w-[calc(100vw-32px)] rounded-lg p-4",
            div { class: "mb-2 flex items-center justify-between",
                h2 { class: "text-base font-bold text-fg", {t("shortcuts.title")} }
                IconButton {
                    icon: "x",
                    label: t("common.close"),
                    shortcut: "Esc",
                    size: IconButtonSize::Sm,
                    onclick: close,
                }
            }
            ul { class: "flex flex-col",
                for def in SHORTCUTS {
                    li {
                        key: "{def.label}",
                        class: "flex min-h-9 items-center justify-between gap-4 border-b border-line py-1 last:border-b-0",
                        span { class: "text-sm text-fg", {t(def.label)} }
                        Kbd { class: "px-2", {t(def.keys)} }
                    }
                }
            }
        }
    }
}
