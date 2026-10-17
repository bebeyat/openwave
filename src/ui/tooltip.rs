//! Pure-CSS tooltip. Appears on hover (after 450ms) and on keyboard focus (immediately).
//! The bubble is `aria-hidden`: the wrapped control must carry its own accessible name
//! (`IconButton` does), so screen readers do not hear it twice.

use dioxus::prelude::*;

use super::kbd::Shortcut;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum TooltipSide {
    #[default]
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum TooltipAlign {
    #[default]
    Center,
    Start,
    End,
}

#[component]
pub fn Tooltip(
    #[props(into)] label: String,
    /// Shortcut hint such as `"Ctrl+K"` or `"Space"`.
    #[props(into, default)]
    shortcut: Option<String>,
    #[props(default)] side: TooltipSide,
    #[props(default)] align: TooltipAlign,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let side = match side {
        TooltipSide::Top => "top",
        TooltipSide::Bottom => "bottom",
    };
    let align = match align {
        TooltipAlign::Center => "center",
        TooltipAlign::Start => "start",
        TooltipAlign::End => "end",
    };
    rsx! {
        span { class: "ow-tip {class}", "data-side": side, "data-align": align,
            {children}
            span {
                class: "ow-tip-bubble pointer-events-none ow-glass flex items-center gap-2 rounded-md px-2.5 py-1.5 text-xs font-semibold text-fg",
                "aria-hidden": "true",
                "{label}"
                if let Some(keys) = shortcut {
                    span { class: "inline-flex items-center gap-1 text-muted",
                        Shortcut { keys }
                    }
                }
            }
        }
    }
}
