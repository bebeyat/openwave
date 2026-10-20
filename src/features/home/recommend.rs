//! Home rows driven by the taste model (`taste.rs`): "Senin için önerilenler" (pooled radio of the
//! user's strongest songs) and "Türlerin" (genre chips + unheard songs of the picked genre).
//! Both paint from their disk cache first and refresh in the background.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use super::row::{RowHeading, RowSkeleton};
use super::sections::TrackCards;
use super::taste::{
    by_avoided_artist, genre_artist_keys, genre_picks, heard_ids, recommend, skipped_artists,
    tastes_with_neighbours, top_genres, weighted_seeds, GenreScore, GENRE_ROW_MAX, RECS_MAX,
    SEED_COUNT, TOP_GENRES,
};
use super::taste_cache::{self as cache, GenreIndex, RecsCache, GENRES_FILE, RECS_FILE};
use crate::backend::api;
use crate::features::tracks::state::library;
use crate::i18n::{t, t_args};
use crate::model::Track;
use crate::store::library::now_ms;
use crate::ui::{Button, ButtonSize, ButtonVariant};

const DAY_MS: u64 = 86_400_000;
/// After a failed genre build, Home waits this long before trying again on its own.
const GENRE_RETRY_AFTER_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Idle,
    Loading,
    Failed,
}

/// Shared by both rows and kept while Home is left and reopened.
static GENRE_INDEX: GlobalSignal<GenreIndex> =
    Signal::global(|| cache::load(&api::data_dir(), GENRES_FILE));
static GENRE_STATUS: GlobalSignal<Status> = Signal::global(|| Status::Idle);
static RECS: GlobalSignal<Vec<Track>> =
    Signal::global(|| cache::load::<RecsCache>(&api::data_dir(), RECS_FILE).tracks);
static GENRE_FAILED_AT: AtomicU64 = AtomicU64::new(0);

/// Rebuild the genre index when it is missing or a week old; at most one build at a time, and
/// not again within [`GENRE_RETRY_AFTER_MS`] of a failure unless `force` (the retry button).
/// The task outlives Home so leaving the page does not strand a half-built index.
fn ensure_genres(force: bool) {
    let now = now_ms();
    let backing_off =
        now.saturating_sub(GENRE_FAILED_AT.load(Ordering::Relaxed)) < GENRE_RETRY_AFTER_MS;
    let fresh = GENRE_INDEX.peek().is_fresh(now);
    if *GENRE_STATUS.peek() == Status::Loading || (!force && (fresh || backing_off)) {
        return;
    }
    *GENRE_STATUS.write() = Status::Loading;
    spawn_forever(async move {
        match cache::build_genres(now).await {
            Some(index) => {
                cache::save(&api::data_dir(), GENRES_FILE, &index);
                *GENRE_INDEX.write() = index;
                *GENRE_STATUS.write() = Status::Idle;
            }
            None => {
                GENRE_FAILED_AT.store(now_ms(), Ordering::Relaxed);
                *GENRE_STATUS.write() = Status::Failed;
            }
        }
    });
}

fn retry_button(onclick: EventHandler<()>) -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Sm,
            icon: "refresh-cw",
            onclick: move |_| onclick.call(()),
            {t("common.retry")}
        }
    }
}

/// Fetch the seeds' radios and rank them; `None` when every radio failed.
async fn build_recs(now: u64) -> Option<RecsCache> {
    let lib = library();
    let seeds = weighted_seeds(&lib.history, &lib.liked, now, SEED_COUNT);
    let mut radios = vec![];
    for (track, weight) in &seeds {
        if let Ok(radio) = api::radio(&track.id).await {
            radios.push((*weight, radio));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if radios.is_empty() {
        return None;
    }
    let index = GENRE_INDEX.peek();
    let neighbours: Vec<Track> = radios.iter().flat_map(|(_, r)| r.iter().cloned()).collect();
    let tastes = tastes_with_neighbours(&lib.history, &lib.liked, &neighbours, now);
    let picked = top_genres(&index.genres, &tastes, TOP_GENRES);
    let avoid = skipped_artists(&lib.history);
    let radios: Vec<(f64, Vec<Track>)> = radios
        .iter()
        .map(|(w, radio)| {
            let kept = radio.iter().filter(|t| !by_avoided_artist(t, &avoid));
            (*w, kept.cloned().collect())
        })
        .collect();
    let tracks = recommend(
        &radios,
        &heard_ids(&lib.history, &lib.liked),
        &genre_artist_keys(&index.genres, &picked),
        RECS_MAX,
    );
    (!tracks.is_empty()).then(|| RecsCache {
        saved_ms: now,
        seed_ids: seeds.iter().map(|(t, _)| t.id.to_string()).collect(),
        tracks,
    })
}

#[component]
pub fn Recommended() -> Element {
    let seed_ids = use_memo(|| {
        let lib = library();
        weighted_seeds(&lib.history, &lib.liked, now_ms(), SEED_COUNT)
            .iter()
            .map(|(t, _)| t.id.to_string())
            .collect::<Vec<String>>()
    });
    let mut status = use_signal(|| Status::Idle);
    let mut nonce = use_signal(|| 0u32);
    // The retry nonce last acted on: a click forces exactly one rebuild, not every later one.
    let handled = use_hook(|| Rc::new(Cell::new(0u32)));
    use_effect(move || {
        let seeds = seed_ids();
        let force = nonce() != handled.get();
        handled.set(nonce.peek().to_owned());
        // Wait for a first genre index so the list gets its genre bonus; re-runs when it lands.
        if seeds.is_empty()
            || GENRE_STATUS() == Status::Loading
            || *status.peek() == Status::Loading
        {
            return;
        }
        let dir = api::data_dir();
        let now = now_ms();
        if !force && cache::load::<RecsCache>(&dir, RECS_FILE).is_fresh(&seeds, now) {
            return;
        }
        status.set(Status::Loading);
        spawn(async move {
            match build_recs(now).await {
                Some(built) => {
                    cache::save(&dir, RECS_FILE, &built);
                    *RECS.write() = built.tracks;
                    status.set(Status::Idle);
                }
                None => status.set(Status::Failed),
            }
        });
    });
    if seed_ids.read().is_empty() {
        return rsx! {};
    }
    let list = RECS();
    rsx! {
        section { class: "mb-10", "aria-labelledby": "ow-home-recs",
            RowHeading { title: t("home.recommended"), id: "ow-home-recs", row: "ow-row-recs",
                if status() == Status::Failed {
                    {retry_button(EventHandler::new(move |_| nonce += 1))}
                }
            }
            if list.is_empty() {
                if status() == Status::Failed {
                    p { class: "text-sm text-muted", {t("home.section_failed")} }
                } else {
                    RowSkeleton { count: 6 }
                }
            } else {
                TrackCards { tracks: list, row_id: "ow-row-recs" }
            }
        }
    }
}

#[component]
fn GenreChip(label: String, share: f64, pressed: bool, onclick: EventHandler<()>) -> Element {
    let pct = (share * 100.0).round().to_string();
    let tone = if pressed {
        "ow-accent-fill"
    } else {
        "border border-line text-muted hover:bg-hover hover:text-fg"
    };
    let aria = t_args(
        "home.genre_chip",
        &[("genre", label.as_str()), ("pct", pct.as_str())],
    );
    rsx! {
        button {
            r#type: "button",
            class: "inline-flex h-9 flex-none items-center gap-2 rounded-full px-4 text-sm font-semibold transition-colors duration-(--ow-dur-hover) {tone}",
            "aria-pressed": "{pressed}",
            "aria-label": "{aria}",
            onclick: move |_| onclick.call(()),
            "{label}"
            span { class: "text-xs font-medium opacity-75 tabular-nums", "aria-hidden": "true", "%{pct}" }
        }
    }
}

/// What the genre section shows; recomputed only when the library, index, recs or chip change.
#[derive(Clone, PartialEq, Default)]
struct GenreView {
    has_taste: bool,
    top: Vec<GenreScore>,
    /// Position in `top` of the shown genre.
    current: usize,
    picks: Vec<Track>,
}

fn genre_view(selected: usize) -> GenreView {
    let lib = library();
    let now = now_ms();
    let index = GENRE_INDEX.read();
    let recs = RECS.read();
    let tastes = tastes_with_neighbours(&lib.history, &lib.liked, &recs, now);
    let top = top_genres(&index.genres, &tastes, TOP_GENRES);
    let current = if selected < top.len() { selected } else { 0 };
    let picks = top
        .get(current)
        .and_then(|g| index.genres.get(g.index))
        .map(|sample| {
            let heard = heard_ids(&lib.history, &lib.liked);
            genre_picks(sample, &heard, &tastes, GENRE_ROW_MAX, now / DAY_MS)
        })
        .unwrap_or_default();
    GenreView {
        has_taste: !tastes.is_empty(),
        top,
        current,
        picks,
    }
}

#[component]
pub fn GenreRows() -> Element {
    use_effect(|| ensure_genres(false));
    let mut selected = use_signal(|| 0usize);
    let view = use_memo(move || genre_view(selected()));
    let view = view.read();
    if !view.has_taste {
        return rsx! {};
    }
    let status = GENRE_STATUS();
    let index_empty = GENRE_INDEX.read().genres.is_empty();
    let row_key = view.top.get(view.current).map_or(0, |g| g.index);
    rsx! {
        section { class: "mb-10", "aria-labelledby": "ow-home-genres",
            RowHeading { title: t("home.genres"), id: "ow-home-genres", row: "ow-row-genre",
                if status == Status::Failed {
                    {retry_button(EventHandler::new(move |_| ensure_genres(true)))}
                }
            }
            if index_empty {
                if status == Status::Failed {
                    p { class: "text-sm text-muted", {t("home.section_failed")} }
                } else {
                    p { class: "mb-3 text-sm text-muted", role: "status", {t("home.genres_building")} }
                    RowSkeleton { count: 6 }
                }
            } else if view.top.is_empty() {
                p { class: "text-sm text-muted", {t("home.genres_empty")} }
            } else {
                div { class: "mb-4 flex flex-wrap gap-2", role: "group", "aria-label": t("home.genres"),
                    for (i, g) in view.top.iter().enumerate() {
                        GenreChip {
                            key: "{g.name}",
                            label: g.name.clone(),
                            share: g.share,
                            pressed: i == view.current,
                            onclick: move |_| selected.set(i),
                        }
                    }
                }
                if view.picks.is_empty() {
                    p { class: "text-sm text-muted", {t("home.genres_all_heard")} }
                } else {
                    // Keyed by genre so switching chips remounts the row (fresh prefetch, scroll reset).
                    div { key: "{row_key}",
                        TrackCards { tracks: view.picks.clone(), row_id: "ow-row-genre" }
                    }
                }
            }
        }
    }
}
