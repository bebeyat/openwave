//! Things a discovery screen does with catalog items: play them, open them, shuffle them.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::i18n::t;
use crate::model::{CollectionKind, Track};
use crate::player::controller;
use crate::routes::Route;
use crate::ui::{toast, toast_error};

/// Album browse ids start with `MPRE` (or `OLAK5uy` for audio playlists); anything else is a playlist.
pub fn collection_kind(id: &str) -> CollectionKind {
    if id.starts_with("MPRE") || id.starts_with("OLAK5uy") {
        CollectionKind::Album
    } else {
        CollectionKind::Playlist
    }
}

pub fn open_artist(id: &str) {
    if !id.is_empty() {
        navigator().push(Route::Artist { id: id.to_owned() });
    }
}

pub fn open_album(id: &str) {
    if !id.is_empty() {
        navigator().push(Route::Album { id: id.to_owned() });
    }
}

/// Deterministic Fisher-Yates shuffle (xorshift); the caller supplies the seed.
pub fn shuffled(tracks: &[Track], seed: u64) -> Vec<Track> {
    let mut out = tracks.to_vec();
    let mut state = seed | 1;
    for i in (1..out.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        out.swap(i, j);
    }
    out
}

/// Play `tracks` in random order.
pub fn play_shuffled(tracks: &[Track]) {
    if !tracks.is_empty() {
        controller::play_now(&shuffled(tracks, crate::player::queue::fresh_seed()), 0);
    }
}

fn play_or_say_empty(tracks: &[Track]) {
    if tracks.is_empty() {
        toast(t("discover.nothing_to_play"));
    } else {
        controller::play_now(tracks, 0);
    }
}

/// Fetch an artist's top songs and play them (hover play button on artist cards).
pub fn play_artist(id: String) {
    spawn(async move {
        match api::artist(&id).await {
            Ok(a) => play_or_say_empty(&a.tracks),
            Err(e) => toast_error(e.user_message()),
        }
    });
}

/// Fetch an album or playlist and play it from the first track.
pub fn play_collection(id: String) {
    spawn(async move {
        match api::collection(collection_kind(&id), &id).await {
            Ok(c) => play_or_say_empty(&c.tracks),
            Err(e) => toast_error(e.user_message()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                id: format!("t{i}").into(),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn shuffle_keeps_every_track_once() {
        let src = tracks(50);
        let mut ids: Vec<_> = shuffled(&src, 12345).iter().map(|t| t.id.clone()).collect();
        ids.sort();
        let mut want: Vec<_> = src.iter().map(|t| t.id.clone()).collect();
        want.sort();
        assert_eq!(ids, want);
    }

    #[test]
    fn shuffle_is_deterministic_and_actually_moves_things() {
        let src = tracks(20);
        assert_eq!(shuffled(&src, 7), shuffled(&src, 7));
        assert_ne!(shuffled(&src, 7), src);
        assert!(shuffled(&[], 1).is_empty());
    }

    #[test]
    fn kind_is_guessed_from_the_id_prefix() {
        assert_eq!(collection_kind("MPREb_abc"), CollectionKind::Album);
        assert_eq!(collection_kind("OLAK5uy_x"), CollectionKind::Album);
        assert_eq!(collection_kind("VLPLabc"), CollectionKind::Playlist);
        assert_eq!(collection_kind("PLabc"), CollectionKind::Playlist);
    }
}
