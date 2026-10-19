//! Finding a playable audio stream: the direct InnerTube chain first, yt-dlp raced in behind it.
//!
//! The direct path answers in a few hundred milliseconds, yt-dlp needs two to three seconds
//! (process start plus a full extraction), so yt-dlp only starts when the direct path failed or is
//! still pending after [`YTDLP_AFTER`]. The first valid stream wins; the loser is dropped (a
//! dropped yt-dlp process is killed, see `ytdlp::command`).
use super::{innertube, ytdlp, Media, MediaError};
use crate::backend::media::stream_url;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// yt-dlp joins the race when the direct lookup has not produced a stream after this long.
pub(super) const YTDLP_AFTER: Duration = Duration::from_millis(1200);
/// A visitor id is renewed after this long.
const VISITOR_TTL: Duration = Duration::from_secs(2 * 3600);
/// A resolved URL is trusted for at most this long, and never closer than `EXPIRY_MARGIN` seconds
/// to its own expiry.
const MAX_MEMO: Duration = Duration::from_secs(3600);
const MIN_MEMO: Duration = Duration::from_secs(300);
const EXPIRY_MARGIN: u64 = 600;

type Found = (innertube::AudioStream, &'static str);

pub(super) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// How long a resolved stream may be reused: until shortly before the signed URL expires (at most
/// an hour); an unknown expiry means five minutes.
pub(super) fn memo_ttl(expires: Option<u64>, now: u64) -> Duration {
    match expires {
        Some(at) => {
            Duration::from_secs(at.saturating_sub(now).saturating_sub(EXPIRY_MARGIN)).min(MAX_MEMO)
        }
        None => MIN_MEMO,
    }
}

fn ytdlp_format(quality: &str) -> &'static str {
    if quality == "economy" {
        "bestaudio[abr<=80]/bestaudio"
    } else {
        "bestaudio[ext=m4a]/bestaudio"
    }
}

fn label(s: innertube::AudioStream) -> Found {
    let via = s.via;
    (s, via)
}

impl Media {
    /// Fetch the visitor id in the background so the first click does not wait for it. This also
    /// opens the connection to www.youtube.com that the first lookup reuses.
    pub async fn warm(self: std::sync::Arc<Self>) {
        let _ = self.visitor().await;
    }

    async fn visitor(&self) -> Option<String> {
        let mut slot = self.visitor.lock().await;
        if let Some((when, id)) = slot.as_ref() {
            if when.elapsed() < VISITOR_TTL {
                return Some(id.clone());
            }
        }
        let id = innertube::fetch_visitor(&self.client).await?;
        *slot = Some((Instant::now(), id.clone()));
        Some(id)
    }

    async fn fast_stream(
        &self,
        key: &str,
        quality: &str,
    ) -> Result<innertube::AudioStream, MediaError> {
        let visitor = self.visitor().await;
        let found = innertube::first_working(
            &self.client,
            key,
            quality,
            visitor.as_deref(),
            innertube::HEDGE_AFTER,
        )
        .await;
        if found.is_err() {
            // A stale id would make every following lookup fail the same way.
            *self.visitor.lock().await = None;
        }
        found
    }

    /// The stream for `key`: direct lookup, with yt-dlp started after [`YTDLP_AFTER`] (or at once
    /// when the direct lookup failed). Returns the stream and the resolver that produced it.
    /// When both fail, the fallback's error is reported (it says whether yt-dlp is missing).
    pub(super) async fn fetch_stream(&self, key: &str, quality: &str) -> Result<Found, MediaError> {
        let fast = self.fast_stream(key, quality);
        tokio::pin!(fast);
        let wait = tokio::time::sleep(YTDLP_AFTER);
        tokio::pin!(wait);
        tokio::select! {
            r = &mut fast => {
                return match r {
                    Ok(s) => Ok(label(s)),
                    Err(_) => self.ytdlp_stream(key, quality).await.map(label),
                };
            }
            () = &mut wait => {}
        }
        if ytdlp::locate().is_none() {
            return fast.await.map(label);
        }
        let slow = self.ytdlp_stream(key, quality);
        tokio::pin!(slow);
        tokio::select! {
            r = &mut fast => match r {
                Ok(s) => Ok(label(s)),
                Err(_) => (&mut slow).await.map(label),
            },
            r = &mut slow => match r {
                Ok(s) => Ok(label(s)),
                Err(y) => (&mut fast).await.map(label).map_err(|_| y),
            },
        }
    }

    pub(super) async fn ytdlp_stream(
        &self,
        key: &str,
        quality: &str,
    ) -> Result<innertube::AudioStream, MediaError> {
        let located = ytdlp::locate().ok_or(MediaError::YtDlpMissing)?;
        let mut cmd = ytdlp::command(&located.path);
        // Older copies on PATH (distro packages) reject this option and would always fail.
        if located.origin == ytdlp::Origin::DataDir {
            cmd.arg("--no-plugin-dirs");
        }
        cmd.args([
            "--ignore-config",
            "--no-cache-dir",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "10",
            "--retries",
            "0",
            "--dump-single-json",
            "-f",
            ytdlp_format(quality),
            &format!("https://www.youtube.com/watch?v={key}"),
        ]);
        let output = tokio::time::timeout(Duration::from_secs(28), cmd.output())
            .await
            .map_err(|_| MediaError::Timeout)?
            .map_err(|_| MediaError::YtDlpMissing)?;
        if !output.status.success() || output.stdout.len() > 5_000_000 {
            return Err(MediaError::Unavailable);
        }
        let meta: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|_| MediaError::BadResponse)?;
        let url = meta["url"]
            .as_str()
            .filter(|s| stream_url(s))
            .ok_or(MediaError::UntrustedSource)?;
        let mime = if meta["ext"] == "webm" {
            "audio/webm"
        } else {
            "audio/mp4"
        };
        Ok(innertube::AudioStream {
            expires: innertube::url_expiry(url),
            url: url.into(),
            mime: mime.into(),
            seconds: meta["duration"].as_f64(),
            via: "ytdlp",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::media::Input;

    fn temp_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("openwave-test-{}", uuid::Uuid::new_v4()))
    }

    async fn media_with_memo(expires: Option<u64>) -> (std::sync::Arc<Media>, String) {
        let media = Media::open(temp_root()).await.unwrap();
        let granted = media.grant(
            Input::Web("https://a.googlevideo.com/x".into()),
            "audio/mp4".into(),
            None,
        );
        media.remember("high/abc".into(), expires, &granted).await;
        (media, "high/abc".into())
    }

    #[tokio::test]
    async fn a_remembered_stream_is_reused_until_forgotten() {
        let (media, key) = media_with_memo(Some(unix_now() + 6 * 3600)).await;
        assert!(media.memo(&key).await.is_some());
        media.forget("abc", "high").await;
        assert!(
            media.memo(&key).await.is_none(),
            "a retry looks the address up again"
        );
    }

    #[tokio::test]
    async fn a_stream_close_to_expiry_is_not_reused() {
        let (media, key) = media_with_memo(Some(unix_now() + 60)).await;
        assert!(media.memo(&key).await.is_none());
    }

    #[tokio::test]
    async fn the_memo_is_bounded_and_drops_the_oldest() {
        let (media, _) = media_with_memo(Some(unix_now() + 6 * 3600)).await;
        for i in 0..(super::super::MAX_MEMOS + 10) {
            let g = media.grant(
                Input::Web("https://a.googlevideo.com/x".into()),
                "audio/mp4".into(),
                None,
            );
            media
                .remember(format!("high/{i}"), Some(unix_now() + 6 * 3600), &g)
                .await;
        }
        assert!(media.resolved.lock().await.len() <= super::super::MAX_MEMOS);
    }

    #[tokio::test]
    async fn concurrent_lookups_of_one_track_share_a_turn() {
        let media = Media::open(temp_root()).await.unwrap();
        let (a, b) = (media.flight("high/k"), media.flight("high/k"));
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        assert!(!std::sync::Arc::ptr_eq(&a, &media.flight("high/other")));
    }

    #[test]
    fn memo_lives_until_shortly_before_the_url_expires() {
        let now = 1_000_000;
        assert_eq!(memo_ttl(Some(now + 6 * 3600), now), MAX_MEMO);
        assert_eq!(
            memo_ttl(Some(now + 1000), now),
            Duration::from_secs(1000 - EXPIRY_MARGIN)
        );
        assert_eq!(memo_ttl(Some(now + 100), now), Duration::ZERO);
        assert_eq!(memo_ttl(Some(now - 5), now), Duration::ZERO);
        assert_eq!(memo_ttl(None, now), MIN_MEMO);
    }
}
