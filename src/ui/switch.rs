//! Switch (on/off) and SwitchRow (label + description + switch).
//!
//! `Switch { checked, on_change, label }`: Space/Enter toggle (native button semantics).

use dioxus::prelude::*;

#[component]
pub fn Switch(
    checked: bool,
    on_change: EventHandler<bool>,
    /// Accessible name (use `labelled_by` instead when a visible label exists).
    #[props(into, default)]
    label: Option<String>,
    #[props(into, default)] labelled_by: Option<String>,
    #[props(default)] disabled: bool,
    #[props(into, default)] id: Option<String>,
) -> Element {
    let track = if checked {
        "ow-accent-fill"
    } else {
        "bg-press"
    };
    let thumb = if checked {
        "translate-x-5 bg-[#fff]"
    } else {
        "translate-x-0.5 bg-fg"
    };
    rsx! {
        button {
            id,
            r#type: "button",
            role: "switch",
            disabled,
            "aria-checked": "{checked}",
            "aria-label": label,
            "aria-labelledby": labelled_by,
            class: "relative inline-flex h-6 w-11 flex-none items-center rounded-full transition-colors duration-(--ow-dur-hover) disabled:opacity-50 {track}",
            onclick: move |_| on_change.call(!checked),
            span { class: "pointer-events-none inline-block h-5 w-5 rounded-full shadow-[0_1px_3px_rgba(0,0,0,0.3)] transition-transform duration-(--ow-dur-hover) ease-ow {thumb}" }
        }
    }
}
