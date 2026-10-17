//! Direct audio-stream lookup through YouTube's InnerTube `/player` endpoint.
//!
//! Approach (public knowledge, as documented by yt-dlp's `ios` client): identify as the official
//! iOS app, POST a small JSON context to `youtubei/v1/player`, and read the plain (unsigned) URLs
//! in `streamingData.adaptiveFormats`. That avoids running a JS interpreter or a yt-dlp process
//! (about 0.3 s instead of several seconds). Any failure makes the caller fall back to yt-dlp.
use super::{error::MediaError, media::stream_url};
use serde_json::{json, Value};
use std::time::Duration;

const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
const ECONOMY_MAX_BITRATE: u64 = 80_000;

/// One public InnerTube client identity (constants as documented by yt-dlp's public source).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientDef {
    pub label: &'static str,
    pub(crate) name: &'static str,
    pub(crate) name_id: &'static str,
    pub(crate) version: &'static str,
    pub(crate) user_agent: &'static str,
    pub(crate) device_make: Option<&'static str>,
    pub(crate) device_model: Option<&'static str>,
    pub(crate) os_name: &'static str,
    pub(crate) os_version: &'static str,
    pub(crate) android_sdk: Option<u32>,
}

pub(crate) const ANDROID_VR: ClientDef = ClientDef {
    label: "android_vr",
    name: "ANDROID_VR",
    name_id: "28",
    version: "1.65.10",
    user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
    device_make: Some("Oculus"),
    device_model: Some("Quest 3"),
    os_name: "Android",
    os_version: "12L",
    android_sdk: Some(32),
};

pub(crate) const VISIONOS: ClientDef = ClientDef {
    label: "visionos",
    name: "VISIONOS",
    name_id: "101",
    version: "1.02",
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
    device_make: Some("Apple"),
    device_model: Some("RealityDevice17,1"),
    os_name: "visionOS",
    os_version: "26.5.23O471",
    android_sdk: None,
};

pub(crate) const IOS: ClientDef = ClientDef {
    label: "ios",
    name: "IOS",
    name_id: "5",
    version: "20.10.4",
    user_agent: "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
    device_make: Some("Apple"),
    device_model: Some("iPhone16,2"),
    os_name: "iPhone",
    os_version: "18.3.2.22D82",
    android_sdk: None,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioStream {
    pub url: String,
    pub mime: String,
    pub seconds: Option<f64>,
    /// Unix seconds at which the signed URL stops working (its `expire` parameter).
    pub expires: Option<u64>,
    /// Which resolver produced it ("visionos", "android_vr", "ios", "ytdlp").
    pub via: &'static str,
}

/// Clients tried in order. VISIONOS serves unthrottled, token-free audio when a visitor id is
/// sent; it is listed twice so that a slow first request is overtaken by an identical second one
/// (measured: about 3 % of lookups take 1 to 2 s, an identical retry takes about 0.2 s). The other
/// two are kept for videos it refuses (they need a PO token for most files, which the tail-range
/// check below detects).
pub(crate) const CHAIN: [ClientDef; 4] = [VISIONOS, VISIONOS, ANDROID_VR, IOS];
/// The next entry of the chain starts when the previous one has not answered after this long.
pub(crate) const HEDGE_AFTER: Duration = Duration::from_millis(600);

/// Unix time at which a signed googlevideo URL expires (its `expire` query parameter).
pub(crate) fn url_expiry(url: &str) -> Option<u64> {
    reqwest::Url::parse(url)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == "expire")
        .and_then(|(_, v)| v.parse().ok())
}

/// An anonymous visitor id (`responseContext.visitorData`); without one the player endpoint
/// answers "sign in to confirm you're not a bot". No cookies, no account.
pub(crate) async fn fetch_visitor(client: &reqwest::Client) -> Option<String> {
    let body = json!({"context": {"client": {
        "clientName": "WEB", "clientVersion": "2.20250101.00.00", "hl": "en", "gl": "US"}}});
    let response = client
        .post("https://www.youtube.com/youtubei/v1/visitor_id?prettyPrint=false")
        .timeout(Duration::from_secs(5))
        .json(&body)
        .send()
        .await
        .ok()?;
    let v: Value = response.json().await.ok()?;
    let id = v["responseContext"]["visitorData"].as_str()?;
    (!id.is_empty() && id.len() < 512).then(|| id.to_owned())
}

/// Try [`CHAIN`] with hedging (see [`hedged`]).
pub(crate) async fn first_working(
    client: &reqwest::Client,
    video_id: &str,
    quality: &str,
    visitor: Option<&str>,
    hedge: Duration,
) -> Result<AudioStream, MediaError> {
    hedged(CHAIN.len(), hedge, |i| {
        audio_stream(client, &CHAIN[i], video_id, quality, visitor)
    })
    .await
}

/// Run `attempt(0..count)` as a hedged chain: attempt `i + 1` starts at once when `i` fails, or
/// after `hedge` when `i` is still pending. The first success wins and the others are dropped
/// (cancelled); when every attempt failed, the last error is returned.
pub(crate) async fn hedged<T, F, Fut>(
    count: usize,
    hedge: Duration,
    attempt: F,
) -> Result<T, MediaError>
where
    F: Fn(usize) -> Fut,
    Fut: std::future::Future<Output = Result<T, MediaError>>,
{
    use futures_util::stream::{FuturesUnordered, StreamExt};
    let mut running = FuturesUnordered::new();
    let mut next = 0;
    let mut last = MediaError::Unavailable;
    loop {
        if running.is_empty() && next < count {
            running.push(attempt(next));
            next += 1;
        }
        if running.is_empty() {
            return Err(last);
        }
        let more = next < count;
        tokio::select! {
            done = running.next() => match done {
                Some(Ok(found)) => return Ok(found),
                Some(Err(e)) => last = e,
                None => {}
            },
            () = tokio::time::sleep(hedge), if more => {
                running.push(attempt(next));
                next += 1;
            }
        }
    }
}

fn request_body(def: &ClientDef, video_id: &str, visitor: Option<&str>) -> Value {
    let mut client = json!({
        "clientName": def.name,
        "clientVersion": def.version,
        "osName": def.os_name,
        "osVersion": def.os_version,
        "userAgent": def.user_agent,
        "hl": "en",
        "gl": "US",
    });
    if let Some(v) = def.device_make {
        client["deviceMake"] = json!(v);
    }
    if let Some(v) = def.device_model {
        client["deviceModel"] = json!(v);
    }
    if let Some(v) = def.android_sdk {
        client["androidSdkVersion"] = json!(v);
    }
    if let Some(v) = visitor {
        client["visitorData"] = json!(v);
    }
    json!({
        "context": {"client": client},
        "videoId": video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
    })
}

/// Best audio-only format for `quality` among `adaptiveFormats` that expose a plain googlevideo URL.
pub(crate) fn pick_format<'a>(player: &'a Value, quality: &str) -> Option<&'a Value> {
    let mut formats: Vec<&Value> = player["streamingData"]["adaptiveFormats"]
        .as_array()?
        .iter()
        .filter(|f| {
            f["mimeType"]
                .as_str()
                .is_some_and(|m| m.starts_with("audio/"))
                && f["url"].as_str().is_some_and(stream_url)
                && f["drmFamilies"].as_array().is_none_or(|d| d.is_empty())
        })
        .collect();
    formats.sort_by_key(|f| std::cmp::Reverse(f["bitrate"].as_u64().unwrap_or(0)));
    if quality == "economy" {
        formats
            .iter()
            .find(|f| f["bitrate"].as_u64().unwrap_or(0) <= ECONOMY_MAX_BITRATE)
            .or(formats.last())
            .copied()
    } else {
        formats.first().copied()
    }
}

/// POST `/youtubei/v1/player` as `def` and return the parsed JSON.
pub(crate) async fn player_response(
    client: &reqwest::Client,
    def: &ClientDef,
    video_id: &str,
    visitor: Option<&str>,
) -> Result<Value, MediaError> {
    let mut request = client.post(PLAYER_URL);
    if let Some(v) = visitor {
        request = request.header("X-Goog-Visitor-Id", v);
    }
    let response = request
        .header("User-Agent", def.user_agent)
        .header("X-YouTube-Client-Name", def.name_id)
        .header("X-YouTube-Client-Version", def.version)
        .header("Origin", "https://www.youtube.com")
        .timeout(Duration::from_secs(8))
        .json(&request_body(def, video_id, visitor))
        .send()
        .await
        .map_err(net)?;
    if !response.status().is_success() {
        return Err(MediaError::Unavailable);
    }
    response.json().await.map_err(|_| MediaError::BadResponse)
}

fn net(e: reqwest::Error) -> MediaError {
    MediaError::Network(e.without_url().to_string())
}

pub async fn audio_stream(
    client: &reqwest::Client,
    def: &ClientDef,
    video_id: &str,
    quality: &str,
    visitor: Option<&str>,
) -> Result<AudioStream, MediaError> {
    let player = player_response(client, def, video_id, visitor).await?;
    let pick = pick_format(&player, quality).ok_or(MediaError::Unavailable)?;
    let url = pick["url"]
        .as_str()
        .ok_or(MediaError::Unavailable)?
        .to_owned();
    let length: u64 = pick["contentLength"]
        .as_str()
        .and_then(|l| l.parse().ok())
        .filter(|l| *l >= 1024)
        .ok_or(MediaError::BadResponse)?;
    // Some URLs only serve the start of a track; make sure the tail is reachable too.
    let tail = client
        .get(&url)
        .header("Range", format!("bytes={}-{}", length - 1024, length - 1))
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(net)?;
    if tail.status().as_u16() != 206 {
        return Err(MediaError::Unavailable);
    }
    let mime = pick["mimeType"]
        .as_str()
        .and_then(|m| m.split(';').next())
        .unwrap_or("audio/mp4")
        .to_owned();
    let seconds = player["videoDetails"]["lengthSeconds"]
        .as_str()
        .and_then(|l| l.parse::<f64>().ok());
    let expires = url_expiry(&url);
    Ok(AudioStream {
        url,
        mime,
        seconds,
        expires,
        via: def.label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(bitrate: u64, mime: &str, url: &str) -> Value {
        json!({"bitrate": bitrate, "mimeType": mime, "url": url, "contentLength": "5000"})
    }

    fn player() -> Value {
        json!({"streamingData": {"adaptiveFormats": [
            format(128_000, "video/mp4; codecs=\"avc1\"", "https://rr1.googlevideo.com/v"),
            format(50_000, "audio/webm; codecs=\"opus\"", "https://rr1.googlevideo.com/low"),
            format(130_000, "audio/mp4; codecs=\"mp4a\"", "https://rr1.googlevideo.com/high"),
            format(256_000, "audio/mp4; codecs=\"mp4a\"", "https://evil.example/untrusted"),
            {"bitrate": 999_000, "mimeType": "audio/mp4", "url": "https://rr1.googlevideo.com/drm", "drmFamilies": ["WIDEVINE"]},
        ]}})
    }

    #[test]
    fn picks_highest_trusted_audio_for_high_quality() {
        let p = player();
        let f = pick_format(&p, "high").unwrap();
        assert_eq!(f["url"], "https://rr1.googlevideo.com/high");
    }

    #[test]
    fn economy_prefers_low_bitrate_and_falls_back_to_lowest() {
        let p = player();
        assert_eq!(
            pick_format(&p, "economy").unwrap()["url"],
            "https://rr1.googlevideo.com/low"
        );
        let only_high = json!({"streamingData": {"adaptiveFormats": [format(130_000, "audio/mp4", "https://a.googlevideo.com/x")]}});
        assert_eq!(
            pick_format(&only_high, "economy").unwrap()["bitrate"],
            130_000
        );
    }

    #[test]
    fn nothing_usable_yields_none() {
        assert!(pick_format(&json!({}), "high").is_none());
        let video_only = json!({"streamingData": {"adaptiveFormats": [format(1, "video/mp4", "https://a.googlevideo.com/x")]}});
        assert!(pick_format(&video_only, "high").is_none());
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    async fn run(plan: Vec<(u64, bool)>, hedge_ms: u64) -> (Result<usize, MediaError>, usize) {
        let started = Arc::new(AtomicUsize::new(0));
        let counter = started.clone();
        let plan = Arc::new(plan);
        let out = hedged(plan.len(), Duration::from_millis(hedge_ms), move |i| {
            counter.fetch_add(1, Ordering::SeqCst);
            let (delay, ok) = plan[i];
            async move {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                if ok {
                    Ok(i)
                } else {
                    Err(MediaError::Unavailable)
                }
            }
        })
        .await;
        (out, started.load(Ordering::SeqCst))
    }

    #[tokio::test]
    async fn a_fast_first_attempt_never_starts_the_second() {
        assert_eq!(run(vec![(5, true), (5, true)], 400).await.0.unwrap(), 0);
        assert_eq!(run(vec![(5, true), (5, true)], 400).await.1, 1);
    }

    #[tokio::test]
    async fn a_failure_starts_the_next_attempt_at_once() {
        let started = std::time::Instant::now();
        let (out, count) = run(vec![(5, false), (10, true)], 2_000).await;
        assert_eq!(out.unwrap(), 1);
        assert_eq!(count, 2);
        assert!(
            started.elapsed() < Duration::from_millis(1_500),
            "no hedge wait"
        );
    }

    #[tokio::test]
    async fn a_slow_attempt_is_overtaken_by_the_hedge() {
        let started = std::time::Instant::now();
        let (out, _) = run(vec![(3_000, true), (10, true)], 60).await;
        assert_eq!(
            out.unwrap(),
            1,
            "the second attempt wins and the first is cancelled"
        );
        assert!(started.elapsed() < Duration::from_millis(1_500));
    }

    #[tokio::test]
    async fn all_failing_reports_an_error_after_trying_everything() {
        let (out, count) = run(vec![(1, false), (1, false), (1, false)], 2_000).await;
        assert!(out.is_err());
        assert_eq!(count, 3);
    }

    #[test]
    fn url_expiry_reads_the_expire_parameter() {
        assert_eq!(
            url_expiry("https://rr1.googlevideo.com/videoplayback?a=b&expire=1792260102&c=d"),
            Some(1_792_260_102)
        );
        assert_eq!(
            url_expiry("https://rr1.googlevideo.com/videoplayback?a=b"),
            None
        );
        assert_eq!(url_expiry("not a url"), None);
    }

    #[test]
    fn every_client_in_the_chain_builds_a_body_with_its_identity() {
        for def in CHAIN {
            let b = request_body(&def, "abcdefghijk", Some("visitor"));
            assert_eq!(b["context"]["client"]["clientName"], def.name);
            assert_eq!(b["context"]["client"]["visitorData"], "visitor");
        }
        assert!(
            request_body(&VISIONOS, "abcdefghijk", None)["context"]["client"]["visitorData"]
                .is_null()
        );
    }

    #[test]
    fn request_identifies_as_ios_client() {
        let b = request_body(&IOS, "abcdefghijk", None);
        assert_eq!(b["context"]["client"]["clientName"], "IOS");
        assert_eq!(b["videoId"], "abcdefghijk");
    }
}
