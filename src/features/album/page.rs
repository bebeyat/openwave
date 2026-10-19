//! `/album/:id`: an album or a catalog playlist (kind is guessed from the id), with its tracks.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::features::shell::pages::BackButton;
use crate::features::tracks::actions::{collection_kind, open_artist, play_shuffled};
use crate::features::tracks::common::ErrorState;
use crate::features::tracks::format::songs_summary;
use crate::features::tracks::hero::{DetailHero, HeroButton};
use crate::features::tracks::playlist_dialog::create_playlist_with;
use crate::features::tracks::TrackList;
use crate::i18n::t;
use crate::model::{CollectionDetails, CollectionKind};
use crate::player::controller;
use crate::ui::{Cover, CoverShape, GradientPlayButton, PlaySize, Skeleton};

fn kind_label(kind: CollectionKind) -> String {
    match kind {
        CollectionKind::Album => t("detail.album"),
        CollectionKind::Playlist => t("detail.playlist"),
    }
}

/// Album tracks come without artwork: give them the album's cover so rows and the player bar
/// are not blank.
pub fn with_track_covers(mut details: CollectionDetails) -> CollectionDetails {
    if let Some(cover) = details.cover.clone() {
        for track in details.tracks.iter_mut().filter(|t| t.cover.is_none()) {
            track.cover = Some(cover.clone());
        }
    }
    details
}

#[component]
fn Loaded(details: CollectionDetails) -> Element {
    let mut saved = use_signal(|| false);
    let total: u64 = details.tracks.iter().map(|t| t.duration_ms).sum();
    let artist = (details.kind == CollectionKind::Album)
        .then(|| {
            details
                .tracks
                .first()
                .and_then(|t| t.artists.first().cloned())
        })
        .flatten();
    let has_tracks = !details.tracks.is_empty();
    let (play_set, shuffle_set, save_set) = (
        details.tracks.clone(),
        details.tracks.clone(),
        details.tracks.clone(),
    );
    let title = details.title.to_string();
    let save_title = title.clone();
    let cover = details.cover.as_ref().map(|c| c.url.to_string());
    rsx! {
        DetailHero {
            backdrop: cover.clone(),
            title,
            byline: artist.map(|a| {
                if a.id.is_empty() {
                    rsx! { span { "{a.name}" } }
                } else {
                    rsx! {
                        button {
                            r#type: "button",
                            class: "rounded-sm hover:underline",
                            onclick: move |_| open_artist(&a.id),
                            "{a.name}"
                        }
                    }
                }
            }),
            meta: format!("{} · {}", kind_label(details.kind), songs_summary(details.tracks.len(), total)),
            art: rsx! { Cover { src: cover, size: 176, alt: "", shape: CoverShape::Card } },
            actions: rsx! {
                GradientPlayButton {
                    size: PlaySize::Md,
                    label: t("common.play"),
                    disabled: !has_tracks,
                    onclick: move |_| controller::play_now(&play_set, 0),
                }
                HeroButton { icon: "shuffle", disabled: !has_tracks, onclick: move |_| play_shuffled(&shuffle_set), {t("common.shuffle")} }
                HeroButton {
                    icon: if saved() { "check" } else { "plus" },
                    disabled: !has_tracks || saved(),
                    onclick: move |_| {
                        create_playlist_with(&save_title, &save_set);
                        saved.set(true);
                    },
                    if saved() { {t("detail.saved")} } else { {t("detail.save_to_library")} }
                }
            },
        }
        TrackList {
            tracks: details.tracks.clone(),
            show_album: details.kind == CollectionKind::Playlist,
            label: details.title.to_string(),
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
pub fn Album(id: String) -> Element {
    let mut nonce = use_signal(|| 0u64);
    let res = use_resource(use_reactive!(|id, nonce| async move {
        let _ = nonce;
        api::collection(collection_kind(&id), &id).await
    }));
    rsx! {
        div { class: "ow-enter ow-page pb-10 pt-4",
            BackButton {}
            // A restarted resource keeps the old value: show the skeleton, not the previous page.
            match &*res.read_unchecked() {
                _ if matches!(*res.state().read(), UseResourceState::Pending) => rsx! { Loading {} },
                None => rsx! { Loading {} },
                Some(Err(e)) => rsx! { ErrorState { message: e.user_message(), on_retry: move |_| nonce += 1 } },
                Some(Ok(details)) => rsx! { Loaded { key: "{id}", details: with_track_covers(details.clone()) } },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cover, Track};

    #[test]
    fn tracks_without_cover_get_the_album_cover() {
        let own = Cover {
            url: "own".into(),
            ..Default::default()
        };
        let details = CollectionDetails {
            cover: Some(Cover {
                url: "album".into(),
                ..Default::default()
            }),
            tracks: vec![
                Track::default(),
                Track {
                    cover: Some(own.clone()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let out = with_track_covers(details);
        assert_eq!(out.tracks[0].cover.as_ref().unwrap().url, "album");
        assert_eq!(out.tracks[1].cover, Some(own));
    }

    #[test]
    fn no_album_cover_changes_nothing() {
        let details = CollectionDetails {
            tracks: vec![Track::default()],
            ..Default::default()
        };
        assert!(with_track_covers(details).tracks[0].cover.is_none());
    }
}
