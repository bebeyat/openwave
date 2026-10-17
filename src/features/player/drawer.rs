//! The queue and lyrics tabs: docked into the shell's right column at >= 1280px (glass panel,
//! 344px) and a 380px slide-over drawer (280ms) below that or in the fullscreen player.
//!
//! It is a non-modal complementary region: the player bar stays usable. Focus moves to the tab
//! list when it opens and returns to the button that opened it when it closes; Esc closes it.

use dioxus::prelude::*;

use super::lyrics::LyricsPane;
use super::queue_list::QueuePanel;
use super::state::{
    docked, DrawerTab, DRAWER, FULLSCREEN, LAST_TAB, LYRICS_BUTTON_ID, QUEUE_BUTTON_ID,
};
use crate::i18n::t;
use crate::ui::dialog::use_presence;
use crate::ui::tabs::tab_id;
use crate::ui::{IconButton, IconButtonSize, TabItem, Tabs};

const DRAWER_MS: u64 = 280;
const GROUP: &str = "drawer";

fn tab_value(tab: DrawerTab) -> &'static str {
    match tab {
        DrawerTab::Queue => "queue",
        DrawerTab::Lyrics => "lyrics",
    }
}

fn parse_tab(value: &str) -> DrawerTab {
    if value == "lyrics" {
        DrawerTab::Lyrics
    } else {
        DrawerTab::Queue
    }
}

#[component]
pub fn Drawer() -> Element {
    let open = DRAWER.read().is_some() && !docked();
    let (mounted, closing) = use_presence(open, DRAWER_MS);
    if !mounted {
        return rsx! {};
    }
    rsx! { DrawerPanel { closing } }
}

/// The docked variant, rendered by the shell's right-panel slot.
#[component]
pub fn DockedPanel() -> Element {
    rsx! {
        aside {
            role: "complementary",
            "aria-label": t("queue.title"),
            "data-slot": "player-panel",
            class: "ow-panel ow-fade-in flex w-[344px] flex-none flex-col overflow-hidden rounded-xl",
            PanelContent { docked: true }
        }
    }
}

#[component]
fn DrawerPanel(closing: bool) -> Element {
    let bottom = if *FULLSCREEN.read() {
        "bottom-2"
    } else {
        "bottom-[104px]"
    };
    // A finished animation with fill-mode `both` still makes the panel the containing block of
    // `position: fixed` descendants (menus, the save dialog), so the class is dropped afterwards.
    let mut settled = use_signal(|| false);
    let anim = if closing {
        "ow-slide-out"
    } else if settled() {
        ""
    } else {
        "ow-slide-in"
    };
    rsx! {
        aside {
            role: "complementary",
            "aria-label": t("queue.title"),
            class: "ow-glass fixed right-2 top-16 z-40 flex w-[380px] max-w-[calc(100%-16px)] flex-col overflow-hidden rounded-xl {bottom} {anim}",
            onanimationend: move |_| settled.set(true),
            PanelContent { docked: false }
        }
    }
}

/// Tabs, close button and the active tab; shared by the docked panel and the drawer.
#[component]
fn PanelContent(docked: bool) -> Element {
    let tab = DRAWER.read().unwrap_or(*LAST_TAB.read());
    let opener = match tab {
        DrawerTab::Queue => QUEUE_BUTTON_ID,
        DrawerTab::Lyrics => LYRICS_BUTTON_ID,
    };
    // Focus follows the active tab (also when the shortcut or a bar button switches it).
    use_effect(use_reactive!(|tab| {
        document::eval(&format!(
            "window.ow.focusId('{}')",
            tab_id(GROUP, tab_value(tab))
        ));
    }));
    use_drop(move || {
        document::eval(&format!(
            "setTimeout(function(){{var a=document.activeElement;if(!a||a===document.body){{var e=document.getElementById('{opener}');if(e)e.focus();}}}},60);"
        ));
    });
    let items = vec![
        TabItem::new("queue", t("queue.title")).icon("list-music"),
        TabItem::new("lyrics", t("lyrics.title")).icon("mic-vocal"),
    ];
    rsx! {
        div { class: "flex flex-none items-center gap-2 px-3 pt-2",
            Tabs {
                class: "flex-1 border-b-0",
                items,
                value: tab_value(tab),
                group: GROUP,
                aria_label: t("queue.title"),
                on_change: move |v: String| {
                    let next = parse_tab(&v);
                    *LAST_TAB.write() = next;
                    *DRAWER.write() = Some(next);
                },
            }
            IconButton {
                icon: "x",
                label: t("queue.close"),
                shortcut: "Esc",
                size: IconButtonSize::Sm,
                onclick: move |_| *DRAWER.write() = None,
            }
        }
        div { class: "h-px flex-none bg-line" }
        div {
            class: "min-h-0 flex-1",
            role: "tabpanel",
            "aria-labelledby": tab_id(GROUP, tab_value(tab)),
            match tab {
                DrawerTab::Queue => rsx! { QueuePanel { hero: docked } },
                DrawerTab::Lyrics => rsx! { LyricsPane { panel: true } },
            }
        }
    }
}
