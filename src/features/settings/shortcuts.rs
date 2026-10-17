//! Klavye kısayolları: the full documented list, shared with the player's "?" popover.

use dioxus::prelude::*;

use super::rows::{Card, SectionTitle};
use crate::features::shell::shortcut_list::SHORTCUTS;
use crate::i18n::t;
use crate::ui::Kbd;

#[component]
pub fn Shortcuts() -> Element {
    rsx! {
        SectionTitle { title: t("settings.section.shortcuts"), intro: t("settings.shortcuts.intro"), icon: "keyboard" }
        Card {
            for def in SHORTCUTS {
                div { key: "{def.label}", class: "flex items-center justify-between gap-6 border-b border-line py-3",
                    span { class: "text-sm text-fg", {t(def.label)} }
                    Kbd { class: "h-7 px-2 text-sm", {t(def.keys)} }
                }
            }
        }
    }
}
