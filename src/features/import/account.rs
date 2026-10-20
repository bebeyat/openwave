//! Importing from the signed-in Spotify account: Liked Songs or one of the user's playlists.

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use super::flow::Ctx;
use crate::backend::api;
use crate::backend::error::Coded;
use crate::backend::import::Source;
use crate::backend::spotify_account::{Pick, UserPlaylist};
use crate::i18n::{t, t_count};
use crate::ui::{Button, ButtonVariant, Spinner};

/// Whether a Spotify sign-in is saved (read once, updated by sign in / sign out).
pub static SIGNED_IN: GlobalSignal<bool> = Signal::global(api::spotify_signed_in);

#[derive(Clone, Debug, PartialEq)]
enum Lists {
    Loading,
    Ready(Vec<UserPlaylist>),
    Failed(String),
}

#[component]
pub fn AccountPicker(ctx: Ctx) -> Element {
    let mut lists = use_signal(|| Lists::Loading);
    use_hook(move || {
        spawn_forever(async move {
            lists.set(match api::spotify_playlists().await {
                Ok(l) => Lists::Ready(l),
                Err(e) => Lists::Failed(e.user_message()),
            });
        });
    });
    let liked = move |_| {
        ctx.submit_source(Source::SpotifyAccount {
            pick: Pick::Liked,
            name: t("import.spotify.liked"),
        })
    };
    rsx! {
        div { class: "ow-card-glass flex flex-col gap-2 px-4 py-3",
            p { class: "text-sm font-semibold text-fg", {t("import.spotify.account")} }
            Button { variant: ButtonVariant::Secondary, icon: "heart", class: "self-start", onclick: liked, {t("import.spotify.liked")} }
            match lists() {
                Lists::Loading => rsx! { Spinner { size: 18 } },
                Lists::Failed(msg) => rsx! { p { class: "text-sm text-muted", "{msg}" } },
                Lists::Ready(items) => rsx! {
                    ul { class: "flex max-h-48 flex-col overflow-y-auto",
                        for p in items {
                            PlaylistRow { key: "{p.id}", ctx, playlist: p.clone() }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn PlaylistRow(ctx: Ctx, playlist: UserPlaylist) -> Element {
    let count = t_count("common.songs", playlist.count);
    let (id, name) = (playlist.id.clone(), playlist.name.clone());
    rsx! {
        li {
            button {
                r#type: "button",
                class: "flex min-h-9 w-full items-center gap-3 rounded-sm px-2 text-left text-sm text-fg hover:bg-hover",
                onclick: move |_| ctx.submit_source(Source::SpotifyAccount {
                    pick: Pick::Playlist(id.clone()),
                    name: name.clone(),
                }),
                span { class: "min-w-0 flex-1 truncate", "{playlist.name}" }
                span { class: "flex-none text-xs text-muted", "{count}" }
            }
        }
    }
}
