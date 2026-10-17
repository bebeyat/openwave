//! Tabs (page sections) and SegmentedControl (compact option switch).
//!
//! * `Tabs { items, value, on_change, aria_label }` + `TabPanel { value, ... }` for the content.
//!   Arrow keys / Home / End move and activate (WAI-ARIA automatic activation).
//! * `SegmentedControl { items, value, on_change, aria_label }` is a radiogroup with the same keys.

use dioxus::prelude::*;

use super::icon::Icon;

#[derive(Clone, PartialEq, Debug)]
pub struct TabItem {
    pub value: String,
    pub label: String,
    pub icon: Option<String>,
}

impl TabItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Index to move to for a navigation key, wrapping at both ends.
pub fn roving_target(key: &Key, current: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        Key::ArrowRight | Key::ArrowDown => Some((current + 1) % len),
        Key::ArrowLeft | Key::ArrowUp => Some((current + len - 1) % len),
        Key::Home => Some(0),
        Key::End => Some(len - 1),
        _ => None,
    }
}

fn dom_id(prefix: &str, group: &str, value: &str) -> String {
    format!("{prefix}-{group}-{value}")
}

/// Id of the tab button for `value`, for `aria-labelledby` on panels.
pub fn tab_id(group: &str, value: &str) -> String {
    dom_id("ow-tab", group, value)
}

#[component]
pub fn Tabs(
    items: Vec<TabItem>,
    #[props(into)] value: String,
    on_change: EventHandler<String>,
    #[props(into)] aria_label: String,
    /// Distinguishes several tab sets on one page; used to build element ids.
    #[props(into, default = "t".to_string())]
    group: String,
    #[props(default)] class: String,
) -> Element {
    let values: Vec<String> = items.iter().map(|i| i.value.clone()).collect();
    let current = values.iter().position(|v| *v == value).unwrap_or(0);
    let group_c = group.clone();
    rsx! {
        div {
            role: "tablist",
            "aria-label": "{aria_label}",
            class: "flex items-center gap-1 border-b border-line {class}",
            onkeydown: move |e| {
                if let Some(next) = roving_target(&e.key(), current, values.len()) {
                    e.prevent_default();
                    on_change.call(values[next].clone());
                    document::eval(&format!(
                        "window.ow.focusId('{}')",
                        tab_id(&group_c, &values[next])
                    ));
                }
            },
            for item in items {
                {
                    let selected = item.value == value;
                    let v = item.value.clone();
                    let tone = if selected {
                        "text-fg border-accent"
                    } else {
                        "text-muted border-transparent hover:text-fg hover:bg-hover"
                    };
                    rsx! {
                        button {
                            key: "{item.value}",
                            id: tab_id(&group, &item.value),
                            r#type: "button",
                            role: "tab",
                            "aria-selected": "{selected}",
                            "aria-controls": dom_id("ow-panel", &group, &item.value),
                            tabindex: if selected { "0" } else { "-1" },
                            class: "-mb-px inline-flex h-10 items-center gap-2 rounded-t-sm border-b-2 px-4 text-sm font-semibold transition-colors duration-(--ow-dur-hover) {tone}",
                            onclick: move |_| on_change.call(v.clone()),
                            if let Some(name) = item.icon.clone() {
                                Icon { name, size: 16 }
                            }
                            "{item.label}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn TabPanel(
    #[props(into)] value: String,
    #[props(into, default = "t".to_string())] group: String,
    #[props(default)] class: String,
    /// Make the panel itself a tab stop. Turn off when it holds focusable controls (APG advice),
    /// otherwise keyboard users tab through a ring around the whole results area.
    #[props(default = true)]
    focusable: bool,
    children: Element,
) -> Element {
    rsx! {
        div {
            id: dom_id("ow-panel", &group, &value),
            role: "tabpanel",
            tabindex: if focusable { Some("0") } else { None },
            "aria-labelledby": tab_id(&group, &value),
            class: "{class}",
            {children}
        }
    }
}

#[component]
pub fn SegmentedControl(
    items: Vec<TabItem>,
    #[props(into)] value: String,
    on_change: EventHandler<String>,
    #[props(into)] aria_label: String,
    #[props(into, default = "s".to_string())] group: String,
    #[props(default)] class: String,
) -> Element {
    let values: Vec<String> = items.iter().map(|i| i.value.clone()).collect();
    let current = values.iter().position(|v| *v == value).unwrap_or(0);
    let group_c = group.clone();
    rsx! {
        div {
            role: "radiogroup",
            "aria-label": "{aria_label}",
            class: "inline-flex items-center gap-1 rounded-full border border-line bg-surface p-1 {class}",
            onkeydown: move |e| {
                if let Some(next) = roving_target(&e.key(), current, values.len()) {
                    e.prevent_default();
                    on_change.call(values[next].clone());
                    document::eval(&format!(
                        "window.ow.focusId('{}')",
                        dom_id("ow-seg", &group_c, &values[next])
                    ));
                }
            },
            for item in items {
                {
                    let selected = item.value == value;
                    let v = item.value.clone();
                    let tone = if selected {
                        "bg-surface-2 text-fg shadow-[inset_0_0_0_1px_var(--ow-border-strong)]"
                    } else {
                        "text-muted hover:text-fg hover:bg-hover"
                    };
                    rsx! {
                        button {
                            key: "{item.value}",
                            id: dom_id("ow-seg", &group, &item.value),
                            r#type: "button",
                            role: "radio",
                            "aria-checked": "{selected}",
                            tabindex: if selected { "0" } else { "-1" },
                            class: "inline-flex h-9 min-w-9 items-center justify-center gap-2 rounded-full px-4 text-sm font-semibold transition-colors duration-(--ow-dur-hover) {tone}",
                            onclick: move |_| on_change.call(v.clone()),
                            if let Some(name) = item.icon.clone() {
                                Icon { name, size: 16 }
                            }
                            "{item.label}"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roving_wraps_and_jumps() {
        assert_eq!(roving_target(&Key::ArrowRight, 2, 3), Some(0));
        assert_eq!(roving_target(&Key::ArrowLeft, 0, 3), Some(2));
        assert_eq!(roving_target(&Key::Home, 2, 3), Some(0));
        assert_eq!(roving_target(&Key::End, 0, 3), Some(2));
        assert_eq!(roving_target(&Key::Enter, 0, 3), None);
        assert_eq!(roving_target(&Key::ArrowRight, 0, 0), None);
    }
}
