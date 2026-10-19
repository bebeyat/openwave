//! TextInput, SearchInput and Select.
//!
//! * `TextInput { label, value, oninput, placeholder, error, hint }`: visible label, error is
//!   announced (`aria-invalid` + `aria-describedby`).
//! * `SearchInput { value, oninput, onsubmit, placeholder, aria_label }`: search icon, clear
//!   button, Enter submits. Give it `id: "ow-global-search"` to join `/` and Ctrl+K focus.
//! * `Select { options, value, on_change, label }`: styled native `<select>` (the OS list is
//!   keyboard- and screen-reader-complete, and follows the dark theme through `color-scheme`).

use dioxus::prelude::*;

use super::cls::next_id;
use super::icon::Icon;

const FIELD: &str = "w-full border text-sm text-fg placeholder:text-placeholder \
transition-colors duration-(--ow-dur-hover) hover:border-line-strong focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent-text \
disabled:opacity-50";

#[component]
pub fn TextInput(
    #[props(into, default)] label: Option<String>,
    #[props(into, default)] aria_label: Option<String>,
    #[props(into)] value: String,
    oninput: EventHandler<String>,
    #[props(into, default)] placeholder: Option<String>,
    #[props(into, default)] hint: Option<String>,
    #[props(into, default)] error: Option<String>,
    #[props(into, default)] icon: Option<String>,
    #[props(into, default = "text".to_string())] kind: String,
    #[props(default)] disabled: bool,
    #[props(into, default)] id: Option<String>,
    #[props(default)] onsubmit: EventHandler<()>,
    #[props(default)] class: String,
) -> Element {
    let auto = use_hook(|| next_id("input"));
    let id = id.unwrap_or(auto);
    let msg_id = format!("{id}-msg");
    let border = if error.is_some() {
        "border-danger-text"
    } else {
        "border-line-strong"
    };
    let pad = if icon.is_some() { "pl-10 pr-3" } else { "px-3" };
    let described = (error.is_some() || hint.is_some()).then(|| msg_id.clone());
    rsx! {
        div { class: "flex flex-col gap-1.5 {class}",
            if let Some(l) = label {
                label { r#for: "{id}", class: "text-sm font-semibold text-fg", "{l}" }
            }
            div { class: "relative",
                if let Some(name) = icon {
                    Icon { name, size: 18, class: "pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" }
                }
                input {
                    id: "{id}",
                    r#type: "{kind}",
                    class: "{FIELD} h-10 rounded-md bg-surface-2 {border} {pad}",
                    value: "{value}",
                    placeholder,
                    disabled,
                    "aria-label": aria_label,
                    "aria-invalid": error.is_some().then_some("true"),
                    "aria-describedby": described,
                    oninput: move |e| oninput.call(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            onsubmit.call(());
                        }
                    },
                }
            }
            if let Some(err) = error.clone() {
                p { id: "{msg_id}", class: "flex items-center gap-1.5 text-sm text-danger-text", role: "alert",
                    Icon { name: "circle-alert", size: 16 }
                    "{err}"
                }
            } else if let Some(h) = hint {
                p { id: "{msg_id}", class: "text-sm text-muted", "{h}" }
            }
        }
    }
}

#[component]
pub fn SearchInput(
    #[props(into)] value: String,
    oninput: EventHandler<String>,
    #[props(default)] onsubmit: EventHandler<String>,
    #[props(default)] onfocus: EventHandler<()>,
    #[props(into, default)] placeholder: Option<String>,
    #[props(into)] aria_label: String,
    #[props(into, default)] id: Option<String>,
    #[props(into, default)] clear_label: Option<String>,
    #[props(default)] class: String,
) -> Element {
    let auto = use_hook(|| next_id("search"));
    let id = id.unwrap_or(auto);
    let clear_label = clear_label.unwrap_or_else(|| crate::i18n::t("common.clear"));
    let has_value = !value.is_empty();
    let submit_value = value.clone();
    rsx! {
        div { class: "relative {class}", role: "search",
            Icon { name: "search", size: 18, class: "pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted" }
            input {
                id: "{id}",
                r#type: "search",
                autocomplete: "off",
                spellcheck: "false",
                class: "{FIELD} ow-panel h-11 rounded-full border-line pl-11 pr-10 [&::-webkit-search-cancel-button]:hidden",
                value: "{value}",
                placeholder,
                "aria-label": "{aria_label}",
                oninput: move |e| oninput.call(e.value()),
                onfocus: move |_| onfocus.call(()),
                onkeydown: move |e| match e.key() {
                    Key::Enter => onsubmit.call(submit_value.clone()),
                    Key::Escape if has_value => {
                        e.stop_propagation();
                        oninput.call(String::new());
                    }
                    _ => {}
                },
            }
            if has_value {
                button {
                    r#type: "button",
                    "aria-label": "{clear_label}",
                    class: "absolute right-0.5 top-1/2 flex h-9 w-9 -translate-y-1/2 items-center justify-center rounded-full text-muted hover:bg-hover hover:text-fg",
                    onclick: move |_| oninput.call(String::new()),
                    Icon { name: "x", size: 16 }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[component]
pub fn Select(
    options: Vec<SelectOption>,
    #[props(into)] value: String,
    on_change: EventHandler<String>,
    #[props(into, default)] label: Option<String>,
    #[props(into, default)] aria_label: Option<String>,
    #[props(default)] disabled: bool,
    #[props(into, default)] id: Option<String>,
    #[props(default)] class: String,
) -> Element {
    let auto = use_hook(|| next_id("select"));
    let id = id.unwrap_or(auto);
    rsx! {
        div { class: "flex flex-col gap-1.5 {class}",
            if let Some(l) = label {
                label { r#for: "{id}", class: "text-sm font-semibold text-fg", "{l}" }
            }
            div { class: "relative",
                select {
                    id: "{id}",
                    class: "{FIELD} h-10 rounded-md bg-surface-2 cursor-pointer appearance-none border-line pl-3 pr-10",
                    disabled,
                    "aria-label": aria_label,
                    onchange: move |e| on_change.call(e.value()),
                    for o in options {
                        option { key: "{o.value}", value: "{o.value}", selected: o.value == value, "{o.label}" }
                    }
                }
                Icon { name: "chevron-down", size: 18, class: "pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-muted" }
            }
        }
    }
}
