//! Library tabs as a pill-shaped segmented control. Still a real `tablist` (roving tabindex,
//! arrow keys, `aria-controls` to the panel); only the look differs from `ui::Tabs`.

use dioxus::prelude::*;

use crate::ui::tabs::{roving_target, tab_id};
use crate::ui::TabItem;

#[component]
pub fn LibraryTabs(
    items: Vec<TabItem>,
    #[props(into)] value: String,
    on_change: EventHandler<String>,
    #[props(into)] aria_label: String,
    #[props(into)] group: String,
    #[props(default)] class: String,
) -> Element {
    let values: Vec<String> = items.iter().map(|i| i.value.clone()).collect();
    let current = values.iter().position(|v| *v == value).unwrap_or(0);
    let group_c = group.clone();
    rsx! {
        div {
            role: "tablist",
            "aria-label": "{aria_label}",
            class: "inline-flex max-w-full items-center gap-1 overflow-x-auto rounded-full border border-line bg-surface p-1 {class}",
            onkeydown: move |e| {
                if let Some(next) = roving_target(&e.key(), current, values.len()) {
                    e.prevent_default();
                    on_change.call(values[next].clone());
                    document::eval(&format!("window.ow.focusId('{}')", tab_id(&group_c, &values[next])));
                }
            },
            for item in items {
                {
                    let selected = item.value == value;
                    let v = item.value.clone();
                    let tone = if selected {
                        "bg-surface-2 text-fg shadow-[inset_0_0_0_1px_var(--ow-border-strong)]"
                    } else {
                        "text-muted hover:bg-hover hover:text-fg"
                    };
                    rsx! {
                        button {
                            key: "{item.value}",
                            id: tab_id(&group, &item.value),
                            r#type: "button",
                            role: "tab",
                            "aria-selected": "{selected}",
                            "aria-controls": "ow-panel-{group}-{item.value}",
                            tabindex: if selected { "0" } else { "-1" },
                            class: "inline-flex h-9 flex-none items-center rounded-full px-5 text-sm font-semibold transition-[background-color,color] duration-(--ow-dur-hover) ease-ow {tone}",
                            onclick: move |_| on_change.call(v.clone()),
                            "{item.label}"
                        }
                    }
                }
            }
        }
    }
}
