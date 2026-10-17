//! `/`: first-run welcome, or the user's own music (recommended, mixes, genres, recent, liked) with an import card.

use dioxus::prelude::*;

use super::logic::{pick_seeds, recent_tracks, MAX_SEEDS, RECENT_MAX};
use super::recommend::{GenreRows, Recommended};
use super::row::RowHeading;
use super::sections::{Mixes, TrackCards, TrySongs};
use super::welcome::{finish_onboarding, ActionCards, WelcomeCard};
use crate::features::shell::pages::Page;
use crate::features::shell::slots::OPEN_IMPORT;
use crate::features::tracks::state::library;
use crate::i18n::t;
use crate::store;
use crate::ui::{Button, ButtonVariant, Icon};

#[component]
fn ImportCard() -> Element {
    rsx! {
        section { class: "mb-4", "aria-label": t("home.import_card.title"),
            div { class: "flex items-center gap-4 rounded-xl border border-line bg-surface p-4",
                span { class: "flex h-11 w-11 flex-none items-center justify-center rounded-lg bg-surface-2 text-accent-text",
                    Icon { name: "download", size: 20 }
                }
                div { class: "min-w-0 flex-1",
                    p { class: "text-sm font-semibold text-fg", {t("home.import_card.title")} }
                    p { class: "text-sm text-muted", {t("home.import_card.body")} }
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    onclick: move |_| *OPEN_IMPORT.write() = true,
                    {t("home.import_card.button")}
                }
            }
        }
    }
}

#[component]
pub fn Home() -> Element {
    let lib = library();
    let mut dismissed = use_signal(|| store::settings().get().onboarding_done);
    let history = &lib.history;
    let has_data = !history.is_empty()
        || !lib.liked.is_empty()
        || !lib.playlists.is_empty()
        || !lib.local_files.is_empty();
    let starter = history.is_empty() && lib.liked.is_empty();
    let seeds = pick_seeds(history, &lib.liked, MAX_SEEDS);
    let recent = recent_tracks(history, RECENT_MAX);
    let liked: Vec<_> = lib.liked.iter().take(RECENT_MAX).cloned().collect();
    let custom = crate::features::settings::boot::CUSTOM.read().clone();
    rsx! {
        Page { title: t("nav.home"),
            div { class: "ow-enter",
            if !has_data && !dismissed() {
                WelcomeCard {
                    on_skip: move |_| {
                        finish_onboarding();
                        dismissed.set(true);
                    },
                }
            }
            if starter {
                section { class: "mb-10", "aria-label": t("home.action.label"),
                    ActionCards {}
                }
                TrySongs {}
            }
            if !seeds.is_empty() {
                if custom.home_recommended {
                    Recommended {}
                }
                if custom.home_mixes {
                    Mixes { seeds }
                }
                if custom.home_genres {
                    GenreRows {}
                }
            }
            if !recent.is_empty() && custom.home_recent {
                section { class: "mb-10", "aria-labelledby": "ow-home-recent",
                    RowHeading { title: t("home.recent"), id: "ow-home-recent", row: "ow-row-recent" }
                    TrackCards { tracks: recent, row_id: "ow-row-recent" }
                }
            }
            if !liked.is_empty() && custom.home_liked {
                section { class: "mb-10", "aria-labelledby": "ow-home-liked",
                    RowHeading { title: t("home.liked"), id: "ow-home-liked", row: "ow-row-liked" }
                    TrackCards { tracks: liked, row_id: "ow-row-liked" }
                }
            }
            if has_data && custom.home_import {
                ImportCard {}
            }
            }
        }
    }
}
