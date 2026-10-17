//! Layout pieces shared by every settings section: a hero header, glass cards of rows. Each row is
//! a label, one line of plain-language help and its control.

use dioxus::prelude::*;

use crate::ui::{next_id, Icon, Switch};

/// Section header: a glass hero card with a gradient icon chip, the title and one line of intro.
#[component]
pub fn SectionTitle(
    #[props(into)] title: String,
    #[props(into, default)] intro: Option<String>,
    #[props(into, default)] icon: Option<String>,
) -> Element {
    rsx! {
        div { class: "ow-card-glass ow-corner-glow ow-span-all mb-5 flex items-center gap-4 px-6 py-5",
            if let Some(name) = icon {
                span { class: "ow-chip-icon h-12 w-12 rounded-lg", "aria-hidden": "true",
                    Icon { name, size: 24 }
                }
            }
            div { class: "min-w-0 flex-1",
                h2 { class: "ow-display text-2xl font-extrabold text-fg", "{title}" }
                if let Some(i) = intro {
                    p { class: "mt-1 text-sm text-muted", "{i}" }
                }
            }
        }
    }
}

/// A glass card holding rows separated by hairlines.
#[component]
pub fn Card(#[props(default)] class: String, children: Element) -> Element {
    rsx! {
        div { class: "ow-card-glass px-3 py-1 [&>*:last-child]:border-b-0 {class}", {children} }
    }
}

/// `label` + `help` on the left, the control on the right. `stack` puts the control below
/// (for wide controls such as a list).
#[component]
pub fn Row(
    #[props(into)] label: String,
    #[props(into, default)] help: Option<String>,
    #[props(default)] stack: bool,
    #[props(into, default)] label_id: Option<String>,
    children: Element,
) -> Element {
    let layout = if stack {
        "flex-col items-stretch gap-3"
    } else {
        // The control moves under the label when the column is too narrow for both.
        "flex-wrap items-center gap-x-6 gap-y-3"
    };
    let control = if stack {
        "flex w-full flex-wrap items-center gap-2"
    } else {
        "flex flex-none flex-wrap items-center justify-end gap-2"
    };
    rsx! {
        div { class: "flex border-b border-line px-3 py-[18px] {layout}",
            div { class: "min-w-[200px] flex-1",
                div { id: label_id, class: "text-sm font-semibold text-fg", "{label}" }
                if let Some(h) = help {
                    div { class: "mt-1 text-sm text-muted", "{h}" }
                }
            }
            div { class: "{control}", {children} }
        }
    }
}

/// A row whose control is an on/off switch.
#[component]
pub fn ToggleRow(
    #[props(into)] label: String,
    #[props(into, default)] help: Option<String>,
    checked: bool,
    on_change: EventHandler<bool>,
) -> Element {
    let id = use_hook(|| next_id("toggle"));
    let label_id = format!("{id}-l");
    rsx! {
        Row { label, help, label_id: label_id.clone(),
            Switch { id, checked, on_change, labelled_by: label_id }
        }
    }
}
