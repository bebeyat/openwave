//! The content sections of Home. Local sections render from the library at once; online ones
//! (mixes, starter songs) show skeletons and fail quietly with a small retry.

use std::time::Duration;

use dioxus::prelude::*;

use super::cache::{self, Mix, MixCache};
use super::logic::{build_mix, featured_query, Seed, TRY_SONGS};
use super::row::{RowHeading, RowSkeleton, ScrollRow};
use crate::backend::api::{self, SearchKind};
use crate::features::tracks::actions::open_album;
use crate::i18n::{language, t, t_args};
use crate::model::Track;
use crate::player::controller;
use crate::store::library::now_ms;
use crate::ui::{Button, ButtonSize, ButtonVariant, MediaCard};

fn cover_of(t: &Track) -> Option<String> {
    t.cover.as_ref().map(|c| c.url.to_string())
}

fn play_label(title: &str) -> String {
    t_args("home.play_card", &[("title", title)])
}

/// A row of track cards: click opens the album (or plays when there is none), hover plays. A
/// played card continues with similar songs (`controller::play_with_similar`).
#[component]
pub fn TrackCards(tracks: Vec<Track>, #[props(into)] row_id: String) -> Element {
    // Look the first cards' stream addresses up now so a click or hover-play starts at once.
    use_hook({
        let tracks = tracks.clone();
        move || controller::prefetch_tracks(&tracks)
    });
    rsx! {
        ScrollRow { id: row_id,
            for (i, track) in tracks.iter().enumerate() {
                {
                    let album = track.album.as_ref().map(|a| a.id.to_string()).filter(|id| !id.is_empty());
                    let one = tracks[i].clone();
                    let play = tracks[i].clone();
                    rsx! {
                        MediaCard {
                            key: "{track.id}",
                            title: track.title.to_string(),
                            subtitle: track.artists_label(),
                            cover: cover_of(track),
                            play_label: play_label(&track.title),
                            on_open: move |_| match &album {
                                Some(id) => open_album(id),
                                None => controller::play_with_similar(&one),
                            },
                            // The other cards of a row are not a playlist: continue with songs
                            // like this one, not with the rest of the row.
                            on_play: move |_| controller::play_with_similar(&play),
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MixStatus {
    Idle,
    Loading,
    Failed,
}

/// Cached mixes at once; a background refresh when the cache is old or the seeds changed.
fn use_mixes(seeds: Vec<Seed>) -> (Signal<Vec<Mix>>, Signal<MixStatus>, Signal<u32>) {
    let mut mixes = use_signal(|| cache::load(&api::data_dir()).mixes);
    let mut status = use_signal(|| MixStatus::Idle);
    let nonce = use_signal(|| 0u32);
    use_effect(use_reactive!(|seeds, nonce| {
        let _ = nonce;
        let ids: Vec<String> = seeds.iter().map(|s| s.id.clone()).collect();
        let dir = api::data_dir();
        if seeds.is_empty() || cache::load(&dir).is_fresh(&ids, now_ms()) {
            return;
        }
        status.set(MixStatus::Loading);
        spawn(async move {
            let mut built = vec![];
            for seed in &seeds {
                if let Ok(radio) = api::radio(&seed.id).await {
                    built.extend(build_mix(seed, radio));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if built.is_empty() {
                status.set(MixStatus::Failed);
                return;
            }
            cache::save(
                &dir,
                &MixCache {
                    saved_ms: now_ms(),
                    mixes: built.clone(),
                },
            );
            mixes.set(built);
            status.set(MixStatus::Idle);
        });
    }));
    (mixes, status, nonce)
}

#[component]
pub fn Mixes(seeds: Vec<Seed>) -> Element {
    let (mixes, status, mut nonce) = use_mixes(seeds.clone());
    if seeds.is_empty() {
        return rsx! {};
    }
    let list = mixes();
    rsx! {
        section { class: "mb-10", "aria-labelledby": "ow-home-mixes",
            RowHeading { title: t("home.mixes"), id: "ow-home-mixes", row: "ow-row-mixes",
                if status() == MixStatus::Failed {
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        icon: "refresh-cw",
                        onclick: move |_| nonce += 1,
                        {t("common.retry")}
                    }
                }
            }
            if list.is_empty() {
                if status() == MixStatus::Failed {
                    p { class: "text-sm text-muted", {t("home.section_failed")} }
                } else {
                    RowSkeleton { count: 6 }
                }
            } else {
                ScrollRow { id: "ow-row-mixes",
                    for mix in list {
                        {
                            let tracks = mix.tracks.clone();
                            let title = t_args("home.mix_of", &[("artist", mix.artist.as_str())]);
                            let play = tracks.clone();
                            rsx! {
                                MediaCard {
                                    key: "{mix.seed_id}",
                                    title: title.clone(),
                                    subtitle: t_args("home.mix_hint", &[("artist", mix.artist.as_str())]),
                                    // Four covers say "this is a list", unlike a single song card.
                                    art: rsx! { crate::features::library::cover::CoverMosaic { mosaic: crate::features::library::logic::mosaic(&mix.tracks) } },
                                    play_label: play_label(&title),
                                    on_open: move |_| controller::play_now(&tracks, 0),
                                    on_play: move |_| controller::play_now(&play, 0),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// "Başlamak için bir şarkı dene": top results of one fixed popular search (online only).
#[component]
pub fn TrySongs() -> Element {
    let mut nonce = use_signal(|| 0u32);
    let res = use_resource(use_reactive!(|nonce| async move {
        let _ = nonce;
        api::search(SearchKind::Track, featured_query(language())).await
    }));
    rsx! {
        section { class: "mb-10", "aria-labelledby": "ow-home-try",
            RowHeading { title: t("home.try_song"), id: "ow-home-try", row: "ow-row-try",
                if matches!(&*res.read_unchecked(), Some(Err(_))) {
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        icon: "refresh-cw",
                        onclick: move |_| nonce += 1,
                        {t("common.retry")}
                    }
                }
            }
            match &*res.read_unchecked() {
                None => rsx! { RowSkeleton { count: TRY_SONGS } },
                Some(Err(_)) => rsx! { p { class: "text-sm text-muted", {t("home.try_song_offline")} } },
                Some(Ok(r)) if r.tracks.is_empty() => rsx! { p { class: "text-sm text-muted", {t("home.section_failed")} } },
                Some(Ok(r)) => rsx! {
                    TrackCards { tracks: r.tracks.iter().take(TRY_SONGS).cloned().collect::<Vec<_>>(), row_id: "ow-row-try" }
                    p { class: "mt-2 text-sm text-muted", {t("home.try_song_hint")} }
                },
            }
        }
    }
}
