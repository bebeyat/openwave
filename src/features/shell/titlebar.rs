//! Window chrome on the ambient backdrop (no separate title strip).
//!
//! The window is created without OS decorations (see `main.rs`). `DragRegion` is an empty element
//! that starts a window drag (double-click toggles maximize) and `WindowControls` holds the three
//! round buttons. The drag region is always a sibling of the buttons, never a parent or overlay,
//! so it cannot swallow clicks.

use dioxus::desktop::tao::event::{Event, WindowEvent};
use dioxus::desktop::{use_window, use_wry_event_handler};
use dioxus::prelude::*;

use crate::i18n::t;
use crate::ui::Icon;

const CTRL: &str = "ow-no-drag flex h-9 w-9 items-center justify-center rounded-full text-muted transition-colors duration-(--ow-dur-hover) hover:bg-hover hover:text-fg active:bg-press";

/// Empty area that drags the window; give it size with `class`.
#[component]
pub fn DragRegion(#[props(default)] class: String) -> Element {
    let window = use_window();
    let w_drag = window.clone();
    rsx! {
        div {
            class: "{class}",
            "aria-hidden": "true",
            onmousedown: move |e| {
                if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                    let _ = w_drag.drag_window();
                }
            },
            ondoubleclick: move |_| {
                let next = !window.is_maximized();
                window.set_maximized(next);
            },
        }
    }
}

/// Minimize, maximize/restore and close, floating on the backdrop at the top right.
#[component]
pub fn WindowControls() -> Element {
    let window = use_window();
    // Alt+F4 and the taskbar close the window without our button; save before it goes.
    use_wry_event_handler(|event, _| {
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            crate::player::session::save_before_exit();
        }
    });
    // Bumped on resize so `is_maximized()` is re-read (maximize via keyboard, snap, double-click).
    let mut tick = use_signal(|| 0u32);
    let _ = tick();
    let maximized = window.is_maximized();
    // Square corners while maximized (the stylesheet rounds the window otherwise).
    use_effect(use_reactive!(|maximized| {
        let js = if maximized {
            "document.documentElement.setAttribute('data-maximized','')"
        } else {
            "document.documentElement.removeAttribute('data-maximized')"
        };
        document::eval(js);
    }));

    let w_max = window.clone();
    let w_min = window.clone();
    let w_close = window.clone();

    let max_label = if maximized {
        t("window.restore")
    } else {
        t("window.maximize")
    };
    let max_icon = if maximized { "minimize-2" } else { "square" };

    rsx! {
        div {
            class: "flex flex-none select-none items-center gap-1",
            onresize: move |_| tick.set(tick() + 1),
            button {
                r#type: "button",
                class: "{CTRL}",
                "aria-label": t("window.minimize"),
                title: t("window.minimize"),
                onclick: move |_| w_min.set_minimized(true),
                Icon { name: "minus", size: 16 }
            }
            button {
                r#type: "button",
                class: "{CTRL}",
                "aria-label": "{max_label}",
                title: "{max_label}",
                onclick: move |_| {
                    let next = !w_max.is_maximized();
                    w_max.set_maximized(next);
                    tick.set(tick() + 1);
                },
                Icon { name: max_icon, size: 14 }
            }
            button {
                r#type: "button",
                class: "ow-no-drag flex h-9 w-9 items-center justify-center rounded-full text-muted transition-colors duration-(--ow-dur-hover) hover:bg-danger-solid hover:text-on-danger",
                "aria-label": t("window.close"),
                title: t("window.close"),
                onclick: move |_| {
                    crate::player::session::save_before_exit();
                    w_close.close();
                },
                Icon { name: "x", size: 16 }
            }
        }
    }
}
