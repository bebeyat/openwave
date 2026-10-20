//! Discord "Listening to" status: one on/off switch (the application id is built in), and
//! signing in to Spotify with the user's own app id.

use std::sync::Arc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use super::rows::{Card, Row, ToggleRow};
use crate::backend::{api, spotify_auth};
use crate::features::import::account::SIGNED_IN;
use crate::i18n::t;
use crate::store::{Settings, SettingsChange};
use crate::ui::{toast, toast_error, Button, ButtonVariant, TextInput};

#[component]
pub fn SpotifyCard(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    let signed_in = *SIGNED_IN.read();
    let mut busy = use_signal(|| false);
    let client_id = settings.spotify_client_id.clone();
    let valid = spotify_auth::valid_client_id(&client_id);
    let sign_in = move |_| {
        let id = client_id.clone();
        busy.set(true);
        spawn_forever(async move {
            match api::spotify_sign_in(id).await {
                Ok(()) => {
                    *SIGNED_IN.write() = true;
                    toast(t("spotify.signed_in"));
                }
                Err(e) => toast_error(t(e.key())),
            }
            busy.set(false);
        });
    };
    let sign_out = move |_| match api::spotify_sign_out() {
        Ok(()) => *SIGNED_IN.write() = false,
        Err(_) => toast_error(t("error.store.write")),
    };
    rsx! {
        Card { class: "mt-5",
            Row { label: t("spotify.client_id"), stack: true,
                TextInput {
                    aria_label: t("spotify.client_id"),
                    value: settings.spotify_client_id.clone(),
                    error: (!valid && !settings.spotify_client_id.is_empty()).then(|| t("spotify.error.client_id")),
                    oninput: move |v: String| on_change.call(SettingsChange::SpotifyClientId(v)),
                }
            }
            Row { label: t("spotify.redirect_uri"),
                code { class: "select-all text-sm text-muted", "{spotify_auth::redirect_uri()}" }
            }
            Row { label: t("spotify.account"),
                if signed_in {
                    Button { variant: ButtonVariant::Secondary, icon: "log-out", onclick: sign_out, {t("spotify.sign_out")} }
                } else {
                    Button { disabled: !valid || busy(), onclick: sign_in, {t("spotify.sign_in")} }
                }
            }
        }
    }
}

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
