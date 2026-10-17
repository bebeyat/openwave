//! `/artist/:id`: top songs, albums and a short bio when the catalog provides one.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::features::shell::pages::BackButton;
use crate::features::tracks::actions::{open_album, play_collection, play_shuffled};
use crate::features::tracks::common::{CardRow, ErrorState};
use crate::features::tracks::hero::{DetailHero, HeroButton};
use crate::features::tracks::TrackList;

use super::radio::start_radio;
use crate::i18n::{t, t_args, t_count};
use crate::model::ArtistDetails;
use crate::player::controller;
use crate::ui::{
    Button, ButtonSize, ButtonVariant, Cover, CoverShape, GradientPlayButton, MediaCard, PlaySize,
    Skeleton,
};

const BIO_LIMIT: usize = 200;

/// First `limit` characters of `bio`, cut at a word boundary with an ellipsis; `None` when short.
pub fn bio_preview(bio: &str, limit: usize) -> Option<String> {
    let bio = bio.trim();
    if bio.chars().count() <= limit {
        return None;
    }
    let cut: String = bio.chars().take(limit).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    Some(format!("{}…", cut.trim_end_matches([',', '.', ';', ' ']),))
}

#[component]
fn Bio(text: String) -> Element {
    let mut open = use_signal(|| false);
    let preview = bio_preview(&text, BIO_LIMIT);
    let shown = match (&preview, open()) {
        (Some(p), false) => p.clone(),
        _ => text.trim().to_owned(),
    };
    rsx! {
        section {
            class: "mb-8 rounded-2xl border border-line bg-surface p-5",
            "aria-labelledby": "ow-artist-about",
            h2 { id: "ow-artist-about", class: "ow-display mb-2 text-xl font-bold text-fg", {t("artist.about")} }
            p { class: "max-w-[720px] text-pretty text-sm leading-relaxed text-muted", "{shown}" }
            if preview.is_some() {
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    class: "-ml-3 mt-2",
                    expanded: open(),
                    onclick: move |_| open.set(!open()),
                    if open() { {t("artist.bio_less")} } else { {t("artist.bio_more")} }
                }
            }
        }
    }
}

#[component]
fn Loaded(details: ArtistDetails) -> Element {
    let has_tracks = !details.tracks.is_empty();
    let (play_set, shuffle_set) = (details.tracks.clone(), details.tracks.clone());
    let radio_seed = details.tracks.first().cloned();
    let radio_busy = use_signal(|| false);
    let cover = details.cover.as_ref().map(|c| c.url.to_string());
    rsx! {
        DetailHero {
            backdrop: cover.clone(),
            title: details.name.to_string(),
            meta: format!("{} · {}", t("detail.artist"), t_count("common.songs", details.tracks.len())),
            art: rsx! { Cover { src: cover, size: 176, alt: "", shape: CoverShape::Round } },
            round_art: true,
            actions: rsx! {
                GradientPlayButton {
                    size: PlaySize::Md,
                    label: t("common.play"),
                    disabled: !has_tracks,
                    onclick: move |_| controller::play_now(&play_set, 0),
                }
                HeroButton { icon: "shuffle", disabled: !has_tracks, onclick: move |_| play_shuffled(&shuffle_set), {t("common.shuffle")} }
                HeroButton {
                    icon: "disc-3",
                    disabled: radio_seed.is_none(),
                    loading: radio_busy(),
                    onclick: move |_| {
                        if let Some(seed) = radio_seed.clone() {
                            start_radio(seed, radio_busy);
                        }
                    },
                    {t("artist.radio")}
                }
            },
        }
        if !details.bio.trim().is_empty() {
            Bio { text: details.bio.clone() }
        }
        if has_tracks {
            section { class: "mb-10", "aria-labelledby": "ow-artist-songs",
                h2 { id: "ow-artist-songs", class: "ow-display mb-3 text-2xl font-extrabold tracking-tight text-fg", {t("artist.top_songs")} }
                TrackList { tracks: details.tracks.clone(), label: t("artist.top_songs") }
            }
        }
        if !details.albums.is_empty() {
            section { "aria-labelledby": "ow-artist-albums",
                h2 { id: "ow-artist-albums", class: "ow-display mb-3 text-2xl font-extrabold tracking-tight text-fg", {t("artist.albums")} }
                CardRow {
                    for a in details.albums.iter() {
                        {
                            let (open_id, play_id) = (a.id.to_string(), a.id.to_string());
                            let subtitle = a.year.map(|y| y.to_string());
                            rsx! {
                                MediaCard {
                                    key: "{a.id}",
                                    title: a.title.to_string(),
                                    subtitle,
                                    cover: a.cover.as_ref().map(|c| c.url.to_string()),
                                    play_label: t_args("home.play_card", &[("title", a.title.as_str())]),
                                    on_open: move |_| open_album(&open_id),
                                    on_play: move |_| play_collection(play_id.clone()),
                                }
                            }
                        }
                    }
                }
            }
        }
        if !has_tracks && details.albums.is_empty() {
            crate::features::tracks::common::NoResults { icon: "music" }
        }
    }
}

#[component]
fn Loading() -> Element {
    rsx! {
        div { role: "status", "aria-busy": "true",
            span { class: "sr-only", {t("common.loading")} }
            div { class: "mb-8 overflow-hidden rounded-2xl",
                Skeleton { class: "h-[280px] w-full" }
            }
        }
        TrackList { tracks: vec![], loading: true }
    }
}

#[component]
pub fn Artist(id: String) -> Element {
    let mut nonce = use_signal(|| 0u64);
    let res = use_resource(use_reactive!(|id, nonce| async move {
        let _ = nonce;
        api::artist(&id).await
    }));
    rsx! {
        div { class: "ow-enter ow-page pb-10 pt-4",
            BackButton {}
            match &*res.read_unchecked() {
                None => rsx! { Loading {} },
                Some(Err(e)) => rsx! { ErrorState { message: e.user_message(), on_retry: move |_| nonce += 1 } },
                Some(Ok(details)) => rsx! { Loaded { details: details.clone() } },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bio_preview;

    #[test]
    fn short_bios_are_not_cut() {
        assert_eq!(bio_preview("Short bio.", 280), None);
    }

    #[test]
    fn long_bios_are_cut_at_a_word_with_an_ellipsis() {
        let bio = "word ".repeat(100);
        let p = bio_preview(&bio, 22).unwrap();
        assert!(p.ends_with('…'));
        assert!(p.chars().count() <= 23);
        assert!(!p.contains("wor…"));
    }

    #[test]
    fn counts_characters_not_bytes() {
        assert_eq!(bio_preview(&"ğ".repeat(10), 10), None);
    }
}
