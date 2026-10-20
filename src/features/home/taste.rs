//! Pure taste model behind "Senin için önerilenler" and the genre rows. No I/O.
//!
//! * Every played track weighs `(1 + ln plays) * 0.5^(age / 14 days)`, halved for each early skip
//!   (up to four); a liked track adds a flat [`LIKE_WEIGHT`]. Artists the user skips at least
//!   [`SKIP_LIMIT`] times, and more often than they listen, are left out of recommendations. The first credited artist gets the full weight, featured artists half.
//! * Artists are matched by lower-cased name and, when known, by catalog id, so imported
//!   (id-less) tracks still count.
//! * A genre's score is the weight of the user's artists found in its sample playlists; an
//!   artist found in several genres splits its weight between them.
//! * Recommendations pool the radio of the strongest seeds: a track scores the seed weight times
//!   how early it appears, summed over seeds, with a bonus for artists of the user's top genres.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::model::{ArtistRef, Track};
use crate::store::library::HistoryEntry;

const DAY_MS: f64 = 86_400_000.0;
pub const HALF_LIFE_DAYS: f64 = 14.0;
pub const LIKE_WEIGHT: f64 = 1.5;
const FEATURED_SHARE: f64 = 0.5;
/// Most a similar-song artist adds, as a share of the user's strongest song.
pub const NEIGHBOUR_SHARE: f64 = 0.25;
/// A genre must hold at least this share of the matched taste to be shown.
pub const MIN_GENRE_SHARE: f64 = 0.08;
pub const TOP_GENRES: usize = 5;
pub const SEED_COUNT: usize = 5;
pub const RECS_MAX: usize = 24;
pub const GENRE_ROW_MAX: usize = 20;
pub const PER_ARTIST_MAX: usize = 2;
const GENRE_BONUS: f64 = 0.25;
const FAMILIAR_BONUS: f64 = 0.6;
const SKIP_FACTOR: f64 = 0.5;
const SKIP_PENALTY_MAX: u32 = 4;
pub const SKIP_LIMIT: u32 = 3;

/// A genre with sample tracks taken from its catalog playlists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenreSample {
    pub name: String,
    pub key: String,
    pub tracks: Vec<Track>,
}

/// One artist of the user's taste: every key it may be matched by, and its weight.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtistTaste {
    pub keys: Vec<String>,
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenreScore {
    /// Index into the genre list the score was computed from.
    pub index: usize,
    pub name: String,
    /// Share of the matched taste, in (0, 1].
    pub share: f64,
}

fn name_key(name: &str) -> Option<String> {
    let n = name.trim().to_lowercase();
    (!n.is_empty()).then(|| format!("n:{n}"))
}

/// Keys an artist can be matched by: its name, plus its catalog id when known.
fn artist_keys(a: &ArtistRef) -> Vec<String> {
    let id = (!a.id.is_empty()).then(|| a.id.to_string());
    name_key(&a.name).into_iter().chain(id).collect()
}

/// Name key of a track's first artist (the diversity cap groups by it).
fn lead_artist(t: &Track) -> Option<String> {
    t.artists.first().and_then(|a| name_key(&a.name))
}

/// Weight of one history entry at `now_ms`.
pub fn play_weight(entry: &HistoryEntry, now_ms: u64) -> f64 {
    let age_days = now_ms.saturating_sub(entry.played_at_ms) as f64 / DAY_MS;
    let skipped = SKIP_FACTOR.powi(entry.skips.min(SKIP_PENALTY_MAX) as i32);
    (1.0 + f64::from(entry.play_count()).ln()) * 0.5_f64.powf(age_days / HALF_LIFE_DAYS) * skipped
}

/// Lead-artist keys the user keeps skipping: at least [`SKIP_LIMIT`] skips, and more skips than plays.
pub fn skipped_artists(history: &[HistoryEntry]) -> HashSet<String> {
    let mut tally: HashMap<String, (u32, u32)> = HashMap::new();
    for h in history {
        if let Some(key) = lead_artist(&h.track) {
            let slot = tally.entry(key).or_default();
            slot.0 += h.skips;
            slot.1 += h.play_count();
        }
    }
    tally
        .into_iter()
        .filter(|(_, (skips, plays))| *skips >= SKIP_LIMIT && skips >= plays)
        .map(|(key, _)| key)
        .collect()
}

/// The track's lead artist is one of `avoid`.
pub fn by_avoided_artist(track: &Track, avoid: &HashSet<String>) -> bool {
    lead_artist(track).is_some_and(|a| avoid.contains(&a))
}

/// Weight per track id: plays (decayed) plus likes. Local files are left out (no catalog match).
fn track_weights<'a>(
    history: &'a [HistoryEntry],
    liked: &'a [Track],
    now_ms: u64,
) -> Vec<(&'a Track, f64)> {
    let mut order: Vec<&Track> = vec![];
    let mut weight: HashMap<&str, f64> = HashMap::new();
    let plays = history.iter().map(|h| (&h.track, play_weight(h, now_ms)));
    let likes = liked.iter().map(|t| (t, LIKE_WEIGHT));
    for (track, w) in plays.chain(likes).filter(|(t, _)| !t.is_local()) {
        let slot = weight.entry(track.id.as_str()).or_insert_with(|| {
            order.push(track);
            0.0
        });
        *slot += w;
    }
    order
        .into_iter()
        .map(|t| (t, weight[t.id.as_str()]))
        .collect()
}

/// Sum weighted artist credits into tastes, merged by name, strongest first.
fn tally<'a>(credits: impl IntoIterator<Item = (&'a ArtistRef, f64)>) -> Vec<ArtistTaste> {
    let mut tastes: Vec<ArtistTaste> = vec![];
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (artist, w) in credits {
        let keys = artist_keys(artist);
        let Some(name) = keys.first().cloned() else {
            continue;
        };
        match by_name.get(&name) {
            Some(&i) => {
                let taste = &mut tastes[i];
                taste.weight += w;
                for k in keys {
                    if !taste.keys.contains(&k) {
                        taste.keys.push(k);
                    }
                }
            }
            None => {
                by_name.insert(name, tastes.len());
                tastes.push(ArtistTaste { keys, weight: w });
            }
        }
    }
    tastes.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    tastes
}

/// Credits of the user's own tracks: the lead artist in full, featured artists at half.
fn own_credits<'a>(
    history: &'a [HistoryEntry],
    liked: &'a [Track],
    now_ms: u64,
) -> Vec<(&'a ArtistRef, f64)> {
    track_weights(history, liked, now_ms)
        .into_iter()
        .flat_map(|(track, w)| {
            track
                .artists
                .iter()
                .enumerate()
                .map(move |(i, a)| (a, if i == 0 { w } else { w * FEATURED_SHARE }))
        })
        .collect()
}

/// The user's artists plus the lead artists of `neighbours` (songs similar to theirs, e.g. the
/// recommendations), each worth at most [`NEIGHBOUR_SHARE`] of the user's strongest song and
/// less further down the list. Widens genre matching when the user's own artists are missing
/// from a genre's sample playlists. Empty when the user has no taste yet.
pub fn tastes_with_neighbours(
    history: &[HistoryEntry],
    liked: &[Track],
    neighbours: &[Track],
    now_ms: u64,
) -> Vec<ArtistTaste> {
    let own = own_credits(history, liked, now_ms);
    let strongest = track_weights(history, liked, now_ms)
        .iter()
        .map(|(_, w)| *w)
        .fold(0.0, f64::max);
    if strongest <= 0.0 {
        return vec![];
    }
    let len = neighbours.len().max(1) as f64;
    let near = neighbours.iter().enumerate().filter_map(|(rank, t)| {
        let w = NEIGHBOUR_SHARE * strongest * (1.0 - rank as f64 / len);
        t.artists.first().map(|a| (a, w))
    });
    tally(own.into_iter().chain(near))
}

fn genre_keys(g: &GenreSample) -> HashSet<String> {
    g.tracks
        .iter()
        .flat_map(|t| t.artists.iter().flat_map(artist_keys))
        .collect()
}

/// The user's genres, strongest first: at most `max`, each holding at least
/// [`MIN_GENRE_SHARE`] of the matched taste. Empty when no artist is found in any genre.
pub fn top_genres(genres: &[GenreSample], tastes: &[ArtistTaste], max: usize) -> Vec<GenreScore> {
    let sets: Vec<HashSet<String>> = genres.iter().map(genre_keys).collect();
    let mut score = vec![0.0; genres.len()];
    for taste in tastes {
        let hits: Vec<usize> = (0..genres.len())
            .filter(|&g| taste.keys.iter().any(|k| sets[g].contains(k)))
            .collect();
        for &g in &hits {
            score[g] += taste.weight / hits.len() as f64;
        }
    }
    let total: f64 = score.iter().sum();
    if total <= 0.0 {
        return vec![];
    }
    let mut ranked: Vec<GenreScore> = score
        .iter()
        .enumerate()
        .map(|(index, s)| GenreScore {
            index,
            name: genres[index].name.clone(),
            share: s / total,
        })
        .filter(|g| g.share >= MIN_GENRE_SHARE)
        .collect();
    ranked.sort_by(|a, b| b.share.total_cmp(&a.share));
    ranked.truncate(max);
    ranked
}

/// The strongest catalog tracks to pool radio from: one per lead artist.
pub fn weighted_seeds(
    history: &[HistoryEntry],
    liked: &[Track],
    now_ms: u64,
    n: usize,
) -> Vec<(Track, f64)> {
    let mut weighted = track_weights(history, liked, now_ms);
    weighted.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut seen = HashSet::new();
    weighted
        .into_iter()
        .filter(|(t, _)| lead_artist(t).is_some_and(|a| seen.insert(a)))
        .take(n)
        .map(|(t, w)| (t.clone(), w))
        .collect()
}

/// Ids the user already played or liked; recommendations skip them.
pub fn heard_ids(history: &[HistoryEntry], liked: &[Track]) -> HashSet<String> {
    history
        .iter()
        .map(|h| &h.track)
        .chain(liked)
        .map(|t| t.id.to_string())
        .collect()
}

/// Every key of every artist in the given genres (for the genre bonus).
pub fn genre_artist_keys(genres: &[GenreSample], picked: &[GenreScore]) -> HashSet<String> {
    picked
        .iter()
        .filter_map(|g| genres.get(g.index))
        .flat_map(genre_keys)
        .collect()
}

/// Keep at most [`PER_ARTIST_MAX`] tracks per lead artist, then `max` in total.
fn diverse(ranked: impl IntoIterator<Item = Track>, max: usize) -> Vec<Track> {
    let mut per_artist: HashMap<String, usize> = HashMap::new();
    ranked
        .into_iter()
        .filter(|t| {
            let n = per_artist
                .entry(lead_artist(t).unwrap_or_default())
                .or_default();
            *n += 1;
            *n <= PER_ARTIST_MAX
        })
        .take(max)
        .collect()
}

/// Pool the radios of weighted seeds into one ranked, unheard, artist-diverse list.
pub fn recommend(
    radios: &[(f64, Vec<Track>)],
    heard: &HashSet<String>,
    genre_artists: &HashSet<String>,
    max: usize,
) -> Vec<Track> {
    let mut order: Vec<Track> = vec![];
    let mut score: HashMap<String, f64> = HashMap::new();
    for (seed_weight, radio) in radios {
        let len = radio.len().max(1) as f64;
        for (rank, track) in radio.iter().enumerate() {
            if heard.contains(track.id.as_str()) || track.id.is_empty() {
                continue;
            }
            let slot = score.entry(track.id.to_string()).or_insert_with(|| {
                order.push(track.clone());
                0.0
            });
            *slot += seed_weight * (1.0 - rank as f64 / len);
        }
    }
    let boosted = |t: &Track| {
        let in_genre = t
            .artists
            .iter()
            .flat_map(artist_keys)
            .any(|k| genre_artists.contains(&k));
        score[t.id.as_str()] * if in_genre { 1.0 + GENRE_BONUS } else { 1.0 }
    };
    // Stable sort: equal scores keep first-seen order.
    order.sort_by(|a, b| boosted(b).total_cmp(&boosted(a)));
    diverse(order, max)
}

/// A small, stable per-day jitter so a genre row changes from day to day but not per render.
fn jitter(id: &str, salt: u64) -> f64 {
    // FNV-1a over the id, mixed with the salt.
    let h = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64 ^ salt, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    (h % 10_000) as f64 / 10_000.0
}

/// Unheard tracks of one genre: artists the user knows come first, the rest shuffled per `salt`.
pub fn genre_picks(
    sample: &GenreSample,
    heard: &HashSet<String>,
    tastes: &[ArtistTaste],
    max: usize,
    salt: u64,
) -> Vec<Track> {
    let known: HashSet<&str> = tastes
        .iter()
        .flat_map(|t| t.keys.iter().map(String::as_str))
        .collect();
    let mut seen = HashSet::new();
    let mut picks: Vec<(f64, &Track)> = sample
        .tracks
        .iter()
        .filter(|t| !t.id.is_empty() && !heard.contains(t.id.as_str()))
        .filter(|t| seen.insert(t.id.as_str()))
        .map(|t| {
            let familiar = t
                .artists
                .iter()
                .flat_map(artist_keys)
                .any(|k| known.contains(k.as_str()));
            let bonus = if familiar { FAMILIAR_BONUS } else { 0.0 };
            (jitter(&t.id, salt) + bonus, t)
        })
        .collect();
    picks.sort_by(|a, b| b.0.total_cmp(&a.0));
    diverse(picks.into_iter().map(|(_, t)| t.clone()), max)
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod skip_tests {
    include!("taste_skip_tests.rs");
}
