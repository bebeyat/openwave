//! "Radio" of an artist: seeded with the artist's top song, plays that song and then similar ones.

use dioxus::prelude::*;

use crate::backend::{api, Coded};
use crate::i18n::t;
use crate::model::Track;
use crate::player::controller;
use crate::ui::toast_error;

/// The seed first, then the similar songs without the seed and without repeats.
pub fn radio_queue(seed: &Track, similar: Vec<Track>) -> Vec<Track> {
    let mut seen = std::collections::HashSet::new();
    seen.insert(seed.id.clone());
    let mut queue = vec![seed.clone()];
    queue.extend(similar.into_iter().filter(|t| seen.insert(t.id.clone())));
    queue
}

/// Look up similar songs for `seed` and start playing; `busy` is true while the lookup runs.
pub fn start_radio(seed: Track, mut busy: Signal<bool>) {
    if busy() {
        return;
    }
    busy.set(true);
    spawn(async move {
        match api::radio(&seed.id).await {
            Ok(similar) if !similar.is_empty() => {
                controller::play_now(&radio_queue(&seed, similar), 0);
            }
            Ok(_) => toast_error(t("artist.radio_empty")),
            Err(e) => toast_error(e.user_message()),
        }
        busy.set(false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn seed_comes_first_and_is_not_repeated() {
        let q = radio_queue(
            &track("s"),
            vec![track("a"), track("s"), track("b"), track("a")],
        );
        let ids: Vec<_> = q.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(ids, ["s", "a", "b"]);
    }

    #[test]
    fn no_similar_songs_still_gives_the_seed() {
        assert_eq!(radio_queue(&track("s"), vec![]).len(), 1);
    }
}
