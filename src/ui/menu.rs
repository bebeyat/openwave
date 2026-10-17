//! DropdownMenu and ContextMenu with shared `MenuItem` / `MenuSeparator`.
//!
//! ```ignore
//! DropdownMenu { label: "Ekle", icon: "plus",
//!     MenuItem { icon: "list-plus", label: "Yeni çalma listesi", onselect: move |_| {} }
//!     MenuSeparator {}
//!     MenuItem { label: "Sil", danger: true, onselect: move |_| {} }
//! }
//! ContextMenu { menu: rsx! { MenuItem { ... } }, div { "right-click me" } }
//! ```
//! Keyboard: Enter/Space/ArrowDown open from the trigger; arrows, Home/End and typeahead move
//! (`window.ow.menuOpen`); Esc and Tab close; focus returns to the trigger.

use dioxus::prelude::*;

use super::button::{Button, ButtonSize, ButtonVariant, IconButton};
use super::cls::next_id;
use super::dialog::use_presence;
use super::icon::Icon;
use super::kbd::Shortcut;

const MENU_MS: u64 = 120;

/// Provided to menu items so a selection can close the menu.
#[derive(Clone, Copy)]
struct MenuCtx {
    close: Callback<()>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum MenuAlign {
    #[default]
    Start,
    End,
}

#[component]
pub fn MenuItem(
    #[props(into)] label: String,
    #[props(into, default)] icon: Option<String>,
    #[props(into, default)] shortcut: Option<String>,
    #[props(default)] danger: bool,
    #[props(default)] disabled: bool,
    #[props(default)] onselect: EventHandler<()>,
) -> Element {
    let ctx = use_context::<MenuCtx>();
    let tone = if danger {
        "text-danger-text"
    } else {
        "text-fg"
    };
    rsx! {
        button {
            r#type: "button",
            role: "menuitem",
            tabindex: "-1",
            disabled,
            class: "flex h-9 w-full items-center gap-3 rounded-md px-3 text-left text-sm font-normal outline-offset-[-2px] hover:bg-hover focus-visible:bg-hover disabled:opacity-50 {tone}",
            onclick: move |_| {
                onselect.call(());
                ctx.close.call(());
            },
            if let Some(name) = icon {
                Icon { name, size: 18 }
            } else {
                span { class: "w-[18px] flex-none" }
            }
            span { class: "min-w-0 flex-1 truncate", "{label}" }
            if let Some(keys) = shortcut {
                span { class: "text-muted", Shortcut { keys } }
            }
        }
    }
}

#[component]
pub fn MenuSeparator() -> Element {
    rsx! { div { role: "separator", class: "my-1 h-px bg-line" } }
}

#[component]
fn MenuPanel(
    id: String,
    label: String,
    close: Callback<()>,
    /// JSON for `window.ow` placement: `{"anchor":id,"align":"start|end"}` or `{"x":..,"y":..}`.
    placement: String,
    closing: bool,
    children: Element,
) -> Element {
    use_context_provider(|| MenuCtx { close });
    let open_id = id.clone();
    use_effect(move || {
        document::eval(&format!("window.ow.menuOpen('{open_id}', {placement})"));
    });
    let drop_id = id.clone();
    use_drop(move || {
        document::eval(&format!("window.ow.menuClose('{drop_id}')"));
    });
    let anim = if closing { "opacity-0" } else { "ow-menu-in" };
    rsx! {
        div {
            id: "{id}",
            role: "menu",
            tabindex: "-1",
            "aria-label": "{label}",
            "data-ow-overlay": "menu",
            class: "fixed z-50 max-h-[calc(100vh-16px)] min-w-[220px] max-w-[320px] overflow-y-auto rounded-lg p-1.5 ow-glass {anim}",
            style: "visibility:hidden;left:0;top:0",
            onkeydown: move |e| match e.key() {
                Key::Escape => {
                    e.stop_propagation();
                    close.call(());
                }
                Key::Tab => close.call(()),
                _ => {}
            },
            {children}
        }
    }
}

/// A trigger button that opens a menu below it.
#[component]
pub fn DropdownMenu(
    /// Visible label. When absent the trigger is an icon button and `aria_label` is required.
    #[props(into, default)]
    label: Option<String>,
    #[props(into, default)] icon: Option<String>,
    /// Accessible name for icon-only triggers.
    #[props(into, default)]
    aria_label: Option<String>,
    #[props(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(default)] align: MenuAlign,
    children: Element,
) -> Element {
    let mut open = use_signal(|| false);
    let menu_id = use_hook(|| next_id("menu"));
    let (mounted, closing) = use_presence(open(), MENU_MS);
    let name = aria_label
        .clone()
        .or_else(|| label.clone())
        .unwrap_or_default();
    let align_attr = match align {
        MenuAlign::Start => "start",
        MenuAlign::End => "end",
    };
    let placement = format!(r#"{{"anchor":"{menu_id}-anchor","align":"{align_attr}"}}"#);
    let close = Callback::new(move |_| open.set(false));
    // The ⋯ trigger morphs into an X while its menu is open.
    let trigger_icon = icon.clone().unwrap_or_else(|| "ellipsis".to_string());
    let trigger_icon = if open() && trigger_icon == "ellipsis" {
        "x".to_string()
    } else {
        trigger_icon
    };
    rsx! {
        span {
            id: "{menu_id}-anchor",
            class: "relative inline-flex",
            onkeydown: move |e| {
                if !open() && matches!(e.key(), Key::ArrowDown | Key::ArrowUp) {
                    e.prevent_default();
                    open.set(true);
                }
            },
            if let Some(text) = label {
                Button {
                    variant,
                    size,
                    icon,
                    expanded: open(),
                    haspopup: "menu",
                    onclick: move |_| {
                        open.set(!open());
                    },
                    "{text}"
                    Icon { name: "chevron-down", size: 16 }
                }
            } else {
                IconButton {
                    icon: trigger_icon,
                    label: name.clone(),
                    expanded: open(),
                    haspopup: "menu",
                    onclick: move |_| {
                        open.set(!open());
                    },
                }
            }
            if mounted {
                div {
                    class: "fixed inset-0 z-40",
                    onclick: move |_| open.set(false),
                    oncontextmenu: move |e| {
                        e.prevent_default();
                        open.set(false);
                    },
                }
                MenuPanel {
                    id: menu_id.clone(),
                    label: name,
                    close,
                    placement,
                    closing,
                    {children}
                }
            }
        }
    }
}

/// Wraps content; right-click (or the Menu key / Shift+F10) opens `menu` at the pointer.
#[component]
pub fn ContextMenu(
    menu: Element,
    #[props(into, default = "Menu".to_string())] label: String,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let mut pos = use_signal(|| (0.0_f64, 0.0_f64));
    let mut open = use_signal(|| false);
    let mut anchor = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let menu_id = use_hook(|| next_id("ctx"));
    let (mounted, closing) = use_presence(open(), MENU_MS);
    let close = Callback::new(move |_| open.set(false));
    let (x, y) = pos();
    rsx! {
        div {
            class: "{class}",
            onmounted: move |e| anchor.set(Some(e.data())),
            oncontextmenu: move |e| {
                e.prevent_default();
                let c = e.client_coordinates();
                pos.set((c.x, c.y));
                open.set(true);
            },
            onkeydown: move |e| {
                let key_menu = e.key() == Key::ContextMenu
                    || (e.key() == Key::F10 && e.modifiers().shift());
                if key_menu {
                    e.prevent_default();
                    if let Some(a) = anchor() {
                        spawn(async move {
                            if let Ok(r) = a.get_client_rect().await {
                                pos.set((r.origin.x + 24.0, r.origin.y + 24.0));
                                open.set(true);
                            }
                        });
                    }
                }
            },
            {children}
            if mounted {
                div {
                    class: "fixed inset-0 z-40",
                    onclick: move |_| open.set(false),
                    oncontextmenu: move |e| {
                        e.prevent_default();
                        open.set(false);
                    },
                }
                MenuPanel {
                    id: menu_id.clone(),
                    label,
                    close,
                    placement: format!(r#"{{"x":{x},"y":{y}}}"#),
                    closing,
                    {menu}
                }
            }
        }
    }
}
