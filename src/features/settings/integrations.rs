//! Discord "Listening to" status: one on/off switch (the application id is built in).

use std::sync::Arc;

use dioxus::prelude::*;

use super::rows::{Card, ToggleRow};
use crate::i18n::t;
use crate::store::{Settings, SettingsChange};

#[component]
pub fn DiscordCard(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    rsx! {
        Card { class: "mt-5",
            ToggleRow {
                label: t("settings.discord.enabled"),
                help: t("settings.discord.enabled.help"),
                checked: settings.discord_enabled,
                on_change: move |v| on_change.call(SettingsChange::DiscordEnabled(v)),
            }
        }
    }
}
