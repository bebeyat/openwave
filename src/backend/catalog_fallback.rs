//! Song search without YouTube Music: when its songs section fails, yt-dlp's `ytsearch` answers
//! instead. After [`MAX_FAILURES`] failures in a row YouTube Music is skipped for [`SKIP_FOR`].
use super::{bad, Entry};
use crate::backend::{error::CatalogError, media::video_key, ytdlp};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_FAILURES: u32 = 3;
const SKIP_FOR: Duration = Duration::from_secs(5 * 60);
const RESULTS: u32 = 20;
const YTDLP_LIMIT: Duration = Duration::from_secs(25);

/// Consecutive YouTube Music failures and, once too many, until when it is skipped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Backoff {
    failures: u32,
    skip_until: Option<Instant>,
}

impl Backoff {
    pub(crate) fn skipping(self, now: Instant) -> bool {
        self.skip_until.is_some_and(|t| now < t)
    }

    /// The state after one YouTube Music attempt.
    pub(crate) fn record(self, ok: bool, now: Instant) -> Backoff {
        if ok {
            return Backoff::default();
        }
        let failures = self.failures + 1;
        if failures >= MAX_FAILURES {
            Backoff {
                failures: 0,
                skip_until: Some(now + SKIP_FOR),
            }
        } else {
            Backoff { failures, ..self }
        }
    }
}

static BACKOFF: Mutex<Backoff> = Mutex::new(Backoff {
    failures: 0,
    skip_until: None,
});

pub(super) fn skipping() -> bool {
    crate::backend::media::locked(&BACKOFF).skipping(Instant::now())
}

pub(super) fn record(ok: bool) {
    let mut b = crate::backend::media::locked(&BACKOFF);
    *b = b.record(ok, Instant::now());
}

/// Song entries of a `yt-dlp --flat-playlist -J ytsearchN:` answer (videos only).
pub(crate) fn youtube_entries(v: &Value) -> Vec<Entry> {
    let Some(list) = v["entries"].as_array() else {
        return vec![];
    };
    list.iter()
        .filter_map(|e| {
            let id = e["id"].as_str().filter(|id| video_key(id))?;
            Some(Entry {
                key: id.to_owned(),
                kind: "track".into(),
                name: e["title"].as_str()?.to_owned(),
                performer: e["channel"]
                    .as_str()
                    .or(e["uploader"].as_str())
                    .unwrap_or("")
                    .to_owned(),
                seconds: e["duration"].as_f64().map_or(0, |d| d.max(0.0) as u64),
                artwork: format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
                artist_key: e["channel_id"].as_str().unwrap_or("").to_owned(),
                ..Entry::default()
            })
        })
        .collect()
}

/// `ytsearch20:<term>` through yt-dlp, as a `tracks` section value.
pub(super) async fn ytdlp_tracks(term: &str) -> Result<Value, CatalogError> {
    let query = format!("ytsearch{RESULTS}:{term}");
    let v = ytdlp::run_json(&["--flat-playlist", "-J", &query], YTDLP_LIMIT)
        .await
        .map_err(bad)?;
    Ok(json!({ "items": youtube_entries(&v) }))
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("catalog_fallback_tests.rs");
}
