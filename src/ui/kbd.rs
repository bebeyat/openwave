//! Keyboard key cap, used in tooltips and the shortcuts list.

use dioxus::prelude::*;

/// `Kbd { "Ctrl" }` renders a single key cap.
#[component]
pub fn Kbd(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        kbd {
            class: "inline-flex h-5 min-w-5 items-center justify-center rounded-sm border border-line-strong px-1 font-sans text-xs font-semibold leading-none text-muted {class}",
            {children}
        }
    }
}

/// Renders a shortcut such as `"Ctrl+K"` as one key cap per part.
#[component]
pub fn Shortcut(#[props(into)] keys: String) -> Element {
    rsx! {
        span { class: "inline-flex items-center gap-1",
            for part in keys.split('+').map(str::to_string).collect::<Vec<_>>() {
                Kbd { key: "{part}", "{part}" }
            }
        }
    }
}
