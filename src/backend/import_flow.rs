//! Import orchestration: prepare (read the source, nothing saved) then run (match tracks with
//! progress). Matching is injected so the flow is testable without a network.
use super::{
    error::ImportError,
    import::{SpotifyPlaylist, SpotifyTrack, SPOTIFY_EMBED_LIMIT},
};
use crate::model::{Cover, Track};
use futures_util::{stream, StreamExt};
use std::{collections::HashSet, future::Future};

const MATCH_CONCURRENCY: usize = 4;

/// What the preview step of the import dialog shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportPreview {
    pub name: String,
    pub count: usize,
    pub cover: Option<Cover>,
    /// `Some(Truncated { limit })` when the source could only be read partially.
    pub warning: Option<ImportError>,
}

#[derive(Clone, Debug, PartialEq)]
enum Payload {
    Spotify(Vec<SpotifyTrack>),
    Ready(Vec<Track>),
}

/// A source that has been read but not yet saved. Pass it to `api::import_run`.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedImport {
    pub preview: ImportPreview,
    payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingTrack {
    pub title: String,
    pub artists: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportResult {
    pub name: String,
    pub tracks: Vec<Track>,
    pub missing: Vec<MissingTrack>,
    pub warning: Option<ImportError>,
}

fn truncation(count: usize) -> Option<ImportError> {
    (count >= SPOTIFY_EMBED_LIMIT).then_some(ImportError::Truncated {
        limit: SPOTIFY_EMBED_LIMIT,
    })
}

pub(super) fn prepare_spotify(list: SpotifyPlaylist) -> Result<PreparedImport, ImportError> {
    if list.tracks.is_empty() {
        return Err(ImportError::PrivateOrEmpty);
    }
    let preview = ImportPreview {
        name: list.name,
        count: list.tracks.len(),
        cover: None,
        warning: truncation(list.tracks.len()),
    };
    Ok(PreparedImport {
        preview,
        payload: Payload::Spotify(list.tracks),
    })
}

pub(super) fn prepare_tracks(
    name: String,
    cover: Option<Cover>,
    tracks: Vec<Track>,
) -> Result<PreparedImport, ImportError> {
    if tracks.is_empty() {
        return Err(ImportError::PrivateOrEmpty);
    }
    let preview = ImportPreview {
        name,
        count: tracks.len(),
        cover,
        warning: None,
    };
    Ok(PreparedImport {
        preview,
        payload: Payload::Ready(tracks),
    })
}

/// Match every Spotify track with `matcher` (4 at a time, results in source order) and report
/// `(done, total)` after each one. Duplicates found twice are kept once.
pub(super) async fn run<M, Fut>(
    prepared: PreparedImport,
    matcher: M,
    progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<ImportResult, ImportError>
where
    M: Fn(SpotifyTrack) -> Fut,
    Fut: Future<Output = Option<Track>>,
{
    let PreparedImport { preview, payload } = prepared;
    let (tracks, missing) = match payload {
        Payload::Ready(tracks) => (tracks, vec![]),
        Payload::Spotify(wanted) => match_all(wanted, &matcher, &progress).await,
    };
    let mut seen = HashSet::new();
    let tracks: Vec<Track> = tracks
        .into_iter()
        .filter(|t| seen.insert(t.id.clone()))
        .collect();
    if tracks.is_empty() {
        return Err(ImportError::NothingMatched);
    }
    Ok(ImportResult {
        name: preview.name,
        tracks,
        missing,
        warning: preview.warning,
    })
}

async fn match_all<M, Fut>(
    wanted: Vec<SpotifyTrack>,
    matcher: &M,
    progress: &(impl Fn(usize, usize) + Send + Sync),
) -> (Vec<Track>, Vec<MissingTrack>)
where
    M: Fn(SpotifyTrack) -> Fut,
    Fut: Future<Output = Option<Track>>,
{
    let total = wanted.len();
    let mut results: Vec<Option<Track>> = vec![None; total];
    let mut done = 0;
    let mut pending = stream::iter(wanted.clone().into_iter().enumerate())
        .map(|(i, want)| async move { (i, matcher(want).await) })
        .buffer_unordered(MATCH_CONCURRENCY);
    while let Some((i, found)) = pending.next().await {
        results[i] = found;
        done += 1;
        progress(done, total);
    }
    let mut tracks = vec![];
    let mut missing = vec![];
    for (want, found) in wanted.into_iter().zip(results) {
        match found {
            Some(t) => tracks.push(t),
            None => missing.push(MissingTrack {
                title: want.title,
                artists: want.artists,
            }),
        }
    }
    (tracks, missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn want(n: usize) -> SpotifyTrack {
        SpotifyTrack {
            title: format!("song {n}"),
            artists: "a".into(),
            duration_ms: 1000,
        }
    }

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Default::default()
        }
    }

    fn playlist(n: usize) -> SpotifyPlaylist {
        SpotifyPlaylist {
            name: "List".into(),
            tracks: (0..n).map(want).collect(),
        }
    }

    #[tokio::test]
    async fn keeps_source_order_and_lists_missing() {
        let prepared = prepare_spotify(playlist(5)).unwrap();
        let calls = AtomicUsize::new(0);
        let last = AtomicUsize::new(0);
        let result = run(
            prepared,
            |w: SpotifyTrack| async move {
                // Later tracks answer first to prove order is by source index.
                let n: u64 = w.title.trim_start_matches("song ").parse().unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(10 * (5 - n))).await;
                (n != 2).then(|| track(&format!("t{n}")))
            },
            |done, total| {
                calls.fetch_add(1, Ordering::SeqCst);
                last.store(done * 100 + total, Ordering::SeqCst);
            },
        )
        .await
        .unwrap();
        let ids: Vec<_> = result.tracks.iter().map(|t| t.id.to_string()).collect();
        assert_eq!(ids, vec!["t0", "t1", "t3", "t4"]);
        assert_eq!(
            result.missing,
            vec![MissingTrack {
                title: "song 2".into(),
                artists: "a".into()
            }]
        );
        assert_eq!(calls.load(Ordering::SeqCst), 5);
        assert_eq!(last.load(Ordering::SeqCst), 505);
        assert_eq!(result.warning, None);
    }

    #[tokio::test]
    async fn nothing_matched_is_an_error() {
        let prepared = prepare_spotify(playlist(3)).unwrap();
        let err = run(prepared, |_| async { None }, |_, _| {})
            .await
            .unwrap_err();
        assert_eq!(err, ImportError::NothingMatched);
    }

    #[tokio::test]
    async fn duplicate_matches_are_collapsed() {
        let prepared = prepare_spotify(playlist(3)).unwrap();
        let result = run(prepared, |_| async { Some(track("same")) }, |_, _| {})
            .await
            .unwrap();
        assert_eq!(result.tracks.len(), 1);
    }

    #[test]
    fn empty_sources_are_private_or_empty() {
        assert_eq!(
            prepare_spotify(playlist(0)).unwrap_err(),
            ImportError::PrivateOrEmpty
        );
        assert_eq!(
            prepare_tracks("x".into(), None, vec![]).unwrap_err(),
            ImportError::PrivateOrEmpty
        );
    }

    #[test]
    fn full_embed_page_warns_about_truncation() {
        let p = prepare_spotify(playlist(SPOTIFY_EMBED_LIMIT)).unwrap();
        assert_eq!(
            p.preview.warning,
            Some(ImportError::Truncated {
                limit: SPOTIFY_EMBED_LIMIT
            })
        );
        assert_eq!(p.preview.count, SPOTIFY_EMBED_LIMIT);
        assert_eq!(prepare_spotify(playlist(10)).unwrap().preview.warning, None);
    }

    #[tokio::test]
    async fn youtube_tracks_pass_through_and_keep_warning() {
        let p = prepare_tracks("Mix".into(), None, vec![track("a"), track("b")]).unwrap();
        assert_eq!(p.preview.count, 2);
        let r = run(p, |_| async { None }, |_, _| {}).await.unwrap();
        assert_eq!(r.name, "Mix");
        assert_eq!(r.tracks.len(), 2);
        assert!(r.missing.is_empty());
    }
}
