//! Button and IconButton.
//!
//! * `Button { variant, size, icon, loading, onclick, "Label" }`
//! * `IconButton { icon, label, shortcut, onclick }`: `label` is required and becomes the
//!   `aria-label` and the tooltip text. Pass `pressed: Some(bool)` for toggles (`aria-pressed`).

use dioxus::prelude::*;

use super::icon::Icon;
use super::tooltip::{Tooltip, TooltipAlign, TooltipSide};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ButtonSize {
    /// 36px: the minimum hit target.
    Sm,
    /// 40px.
    #[default]
    Md,
    /// 48px.
    Lg,
}

const BASE: &str = "inline-flex flex-none select-none items-center justify-center gap-2 whitespace-nowrap font-semibold \
transition-[background-color,filter,transform,color,box-shadow] duration-(--ow-dur-hover) ease-ow \
active:scale-[0.97] active:duration-(--ow-dur-press) aria-disabled:pointer-events-none aria-disabled:opacity-50 disabled:pointer-events-none disabled:opacity-50";

fn variant_class(v: ButtonVariant) -> &'static str {
    match v {
        ButtonVariant::Primary => "ow-accent-fill hover:brightness-110 active:brightness-95",
        ButtonVariant::Secondary => {
            "border border-line bg-surface-2 text-fg hover:bg-hover active:bg-press"
        }
        ButtonVariant::Ghost => "bg-transparent text-fg hover:bg-hover active:bg-press",
        ButtonVariant::Danger => {
            "bg-danger-solid text-on-danger hover:brightness-95 active:brightness-90"
        }
    }
}

fn size_class(s: ButtonSize) -> &'static str {
    match s {
        ButtonSize::Sm => "h-9 rounded-full px-4 text-sm",
        ButtonSize::Md => "h-10 rounded-full px-5 text-sm",
        ButtonSize::Lg => "h-12 rounded-full px-7 text-base",
    }
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    /// Lucide icon name shown before the label.
    #[props(into, default)]
    icon: Option<String>,
    /// Shows a spinner and ignores clicks, keeping the label visible.
    #[props(default)]
    loading: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    #[props(into, default)] id: Option<String>,
    #[props(default)] class: String,
    /// Only needed when the visible content is not text.
    #[props(into, default)]
    aria_label: Option<String>,
    #[props(default)] pressed: Option<bool>,
    #[props(default)] expanded: Option<bool>,
    #[props(into, default)] haspopup: Option<String>,
    #[props(default)] autofocus: bool,
    children: Element,
) -> Element {
    let classes = format!(
        "{BASE} {} {} {class}",
        variant_class(variant),
        size_class(size)
    );
    let icon_size = if size == ButtonSize::Lg { 20 } else { 18 };
    let inactive = loading || disabled;
    rsx! {
        button {
            id,
            r#type: "button",
            class: "{classes}",
            disabled,
            autofocus,
            "data-autofocus": autofocus.then_some("true"),
            "aria-disabled": loading.then_some("true"),
            "aria-busy": loading.then_some("true"),
            "aria-label": aria_label,
            "aria-pressed": pressed.map(|p| p.to_string()),
            "aria-expanded": expanded.map(|p| p.to_string()),
            "aria-haspopup": haspopup,
            onclick: move |e| {
                if !inactive {
                    onclick.call(e);
                }
            },
            if loading {
                Icon { name: "loader-circle", size: icon_size, class: "ow-spin" }
            } else if let Some(name) = icon {
                Icon { name, size: icon_size }
            }
            {children}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum IconButtonSize {
    /// 36px.
    Sm,
    /// 40px.
    #[default]
    Md,
    /// 56px (play/pause).
    Xl,
}

#[component]
pub fn IconButton(
    #[props(into)] icon: String,
    /// Required accessible name; also the tooltip text.
    #[props(into)]
    label: String,
    #[props(into, default)] shortcut: Option<String>,
    #[props(default = ButtonVariant::Ghost)] variant: ButtonVariant,
    #[props(default)] size: IconButtonSize,
    #[props(default)] disabled: bool,
    #[props(default)] pressed: Option<bool>,
    #[props(default)] expanded: Option<bool>,
    #[props(into, default)] haspopup: Option<String>,
    #[props(default)] filled: bool,
    /// Small dot under the icon: a second, non-color cue that a toggle is on.
    #[props(default)]
    indicator: bool,
    #[props(default)] tooltip_side: TooltipSide,
    #[props(default)] tooltip_align: TooltipAlign,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    #[props(into, default)] id: Option<String>,
    #[props(default)] class: String,
) -> Element {
    let (box_class, icon_px) = match size {
        IconButtonSize::Sm => ("h-9 w-9", 18),
        IconButtonSize::Md => ("h-10 w-10", 20),
        IconButtonSize::Xl => ("h-14 w-14", 26),
    };
    // A toggled-on ghost button gets an accent tint so state never relies on color alone
    // (aria-pressed is exposed too, and the icon may be swapped by the caller).
    let on = pressed == Some(true) && variant == ButtonVariant::Ghost;
    let state = if on { "text-accent-text bg-hover" } else { "" };
    let rel = if indicator { "relative" } else { "" };
    let halo = if variant == ButtonVariant::Primary && size == IconButtonSize::Xl {
        "ow-accent-glow"
    } else {
        ""
    };
    let classes = format!(
        "{BASE} {} {box_class} rounded-full {rel} {state} {halo} {class}",
        variant_class(variant)
    );
    let keys = shortcut.clone();
    rsx! {
        Tooltip { label: label.clone(), shortcut: keys, side: tooltip_side, align: tooltip_align,
            button {
                id,
                r#type: "button",
                class: "{classes}",
                disabled,
                "aria-label": "{label}",
                "aria-pressed": pressed.map(|p| p.to_string()),
                "aria-expanded": expanded.map(|p| p.to_string()),
                "aria-haspopup": haspopup,
                "aria-keyshortcuts": shortcut,
                onclick: move |e| onclick.call(e),
                Icon { name: icon, size: icon_px, filled }
                if indicator {
                    span {
                        class: "absolute bottom-1 left-1/2 h-1 w-1 -translate-x-1/2 rounded-full bg-accent-text",
                        "aria-hidden": "true",
                    }
                }
            }
        }
    }
}
