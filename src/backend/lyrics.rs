//! Lyrics: an opt-in LRCLIB client and an LRC parser.
//!
//! Privacy: [`fetch`] makes no request at all unless `enabled` is true. When enabled it sends
//! only the track title, artist, album and duration to `lrclib.net`.
use super::error::LyricsError;
use crate::model::{LyricLine, LyricWord, Lyrics};
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};

pub const LRCLIB_BASE: &str = "https://lrclib.net";
const MAX_FIELD_LEN: usize = 600;

/// What to look up. `duration_secs` greatly improves matching when known.
#[derive(Clone, Debug, Default)]
pub struct LyricsQuery<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: Option<&'a str>,
    pub duration_secs: Option<f64>,
}

fn http() -> Result<&'static reqwest::Client, LyricsError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| LyricsError::Network(e.clone()))
}

/// Look lyrics up on LRCLIB. Returns `Err(Disabled)` without any network access when the user has
/// not enabled lyrics, and `Ok(None)` when nothing was found (or the track is instrumental).
pub async fn fetch(enabled: bool, q: &LyricsQuery<'_>) -> Result<Option<Lyrics>, LyricsError> {
    fetch_from(LRCLIB_BASE, enabled, q).await
}

pub(crate) async fn fetch_from(
    base: &str,
    enabled: bool,
    q: &LyricsQuery<'_>,
) -> Result<Option<Lyrics>, LyricsError> {
    if !enabled {
        return Err(LyricsError::Disabled);
    }
    let title = q.title.trim();
    let artist = q.artist.trim();
    if title.is_empty() || title.len() > MAX_FIELD_LEN || artist.len() > MAX_FIELD_LEN {
        return Err(LyricsError::InvalidInput);
    }
    let duration = q
        .duration_secs
        .filter(|s| s.is_finite() && *s > 0.0 && *s < 10_800.0)
        .map(|s| s.round() as u64);
    let mut params = vec![
        ("track_name", title.to_owned()),
        ("artist_name", artist.to_owned()),
    ];
    if let Some(album) = q
        .album
        .map(str::trim)
        .filter(|a| !a.is_empty() && a.len() <= MAX_FIELD_LEN)
    {
        params.push(("album_name", album.to_owned()));
    }
    if let Some(d) = duration {
        params.push(("duration", d.to_string()));
    }
    match get_json(&format!("{base}/api/get"), &params).await {
        Ok(Some(v)) => Ok(from_record(&v)),
        Ok(None) => search_fallback(base, &params, duration).await,
        // The exact lookup sometimes fails server-side (LRCLIB answers 503 for some tracks)
        // while search still has them; the first error is reported only if search finds nothing.
        Err(e) => match search_fallback(base, &params, duration).await {
            Ok(Some(found)) => Ok(Some(found)),
            _ => Err(e),
        },
    }
}

async fn get_json(url: &str, params: &[(&str, String)]) -> Result<Option<Value>, LyricsError> {
    let res = http()?
        .get(url)
        .query(params)
        .send()
        .await
        .map_err(|e| LyricsError::Network(e.to_string()))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::BAD_REQUEST {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(LyricsError::Unavailable);
    }
    res.json()
        .await
        .map(Some)
        .map_err(|_| LyricsError::Unavailable)
}

async fn search_fallback(
    base: &str,
    params: &[(&str, String)],
    duration: Option<u64>,
) -> Result<Option<Lyrics>, LyricsError> {
    let wanted: Vec<_> = params
        .iter()
        .filter(|(k, _)| *k != "duration")
        .cloned()
        .collect();
    let Some(Value::Array(list)) = get_json(&format!("{base}/api/search"), &wanted).await? else {
        return Ok(None);
    };
    let usable = |v: &&Value| {
        let near = match (duration, v["duration"].as_f64()) {
            (Some(want), Some(got)) => (got - want as f64).abs() <= 3.0,
            _ => true,
        };
        near && from_record(v).is_some()
    };
    let best = list.iter().filter(usable).max_by_key(|v| {
        v["syncedLyrics"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
    });
    Ok(best.and_then(from_record))
}

fn from_record(v: &Value) -> Option<Lyrics> {
    if v["instrumental"].as_bool() == Some(true) {
        return None;
    }
    let synced = v["syncedLyrics"].as_str().unwrap_or("");
    let plain = v["plainLyrics"].as_str().unwrap_or("");
    let mut out = parse(if synced.trim().is_empty() {
        plain
    } else {
        synced
    });
    if out.plain.is_empty() {
        out.plain = plain.trim().to_owned();
    }
    out.provider = "LRCLIB".into();
    (!out.is_empty()).then_some(out)
}

/// Parse `[mm:ss.xx]`, `[mm:ss]` or `[mm:ss:xx]` (also the inside of `<…>` word tags) to ms.
/// The fraction is read digit by digit, so `.5` is 500 ms and `.05` is 50 ms.
pub fn parse_time(tag: &str) -> Option<u64> {
    let parts: Vec<&str> = tag.trim().split(':').collect();
    let all_digits =
        |s: &str, max: usize| (1..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
    let (minutes, secs, frac) = match parts.as_slice() {
        [m, s] => {
            let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
            (*m, whole, frac)
        }
        [m, s, f] => (*m, *s, *f),
        _ => return None,
    };
    if !all_digits(minutes, 4) || !all_digits(secs, 2) || !(frac.is_empty() || all_digits(frac, 9))
    {
        return None;
    }
    let ms: u64 = frac
        .bytes()
        .take(3)
        .chain(std::iter::repeat(b'0'))
        .take(3)
        .fold(0, |n, b| n * 10 + u64::from(b - b'0'));
    Some(minutes.parse::<u64>().ok()? * 60_000 + secs.parse::<u64>().ok()? * 1000 + ms)
}

enum Tag {
    Time(u64),
    Offset(i64),
    Meta,
}

fn classify(inner: &str) -> Option<Tag> {
    if let Some(ms) = parse_time(inner) {
        return Some(Tag::Time(ms));
    }
    let (key, value) = inner.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    if key.eq_ignore_ascii_case("offset") {
        return Some(value.trim().parse::<i64>().map_or(Tag::Meta, Tag::Offset));
    }
    Some(Tag::Meta)
}

/// Leading `[…]` groups of a line: (tags, remaining text, whether a metadata tag was seen).
fn leading_tags(line: &str) -> (Vec<Tag>, &str) {
    let mut tags = vec![];
    let mut rest = line.trim();
    while let Some((inner, tail)) = rest.strip_prefix('[').and_then(|r| r.split_once(']')) {
        let Some(tag) = classify(inner) else { break };
        tags.push(tag);
        rest = tail.trim_start();
    }
    (tags, rest.trim_end())
}

/// Split enhanced text (`<00:01.00>Hel<00:01.50>lo`) into plain text and word timings. A word ends
/// where the next begins; the last word's end is `0` for the caller to fill in.
fn split_words(src: &str) -> (String, Vec<LyricWord>) {
    let mut plain = String::new();
    let mut words: Vec<LyricWord> = vec![];
    let mut rest = src;
    while let Some(i) = rest.find('<') {
        let (before, tag_and_tail) = rest.split_at(i);
        plain.push_str(before);
        if let Some(w) = words.last_mut() {
            w.text.push_str(before);
        }
        let parsed = tag_and_tail[1..]
            .split_once('>')
            .and_then(|(t, tail)| parse_time(t).map(|ms| (ms, tail)));
        match parsed {
            Some((ms, tail)) => {
                if let Some(w) = words.last_mut() {
                    w.end_ms = ms;
                }
                words.push(LyricWord {
                    start_ms: ms,
                    end_ms: 0,
                    text: String::new(),
                });
                rest = tail;
            }
            None => {
                plain.push('<');
                if let Some(w) = words.last_mut() {
                    w.text.push('<');
                }
                rest = &tag_and_tail[1..];
            }
        }
    }
    plain.push_str(rest);
    if let Some(w) = words.last_mut() {
        w.text.push_str(rest);
    }
    words.retain(|w| !w.text.trim().is_empty());
    (plain.trim().to_owned(), words)
}

fn shift(ms: u64, offset: i64) -> u64 {
    // A positive offset makes lyrics appear earlier, so it is subtracted from every timestamp.
    (ms as i64).saturating_sub(offset).max(0) as u64
}

/// Parse LRC text into time-sorted lines. Lines may carry several stamps (`[00:10][00:50]text`),
/// metadata tags (`[ar:…]`) are skipped and `[offset:±ms]` is applied.
pub fn parse_lrc(src: &str) -> Vec<LyricLine> {
    let src = src.trim_start_matches('\u{feff}');
    let mut offset = 0i64;
    let mut lines: Vec<(Vec<u64>, String)> = vec![];
    for raw in src.lines() {
        let (tags, text) = leading_tags(raw);
        let mut stamps = vec![];
        for tag in &tags {
            match tag {
                Tag::Time(ms) => stamps.push(*ms),
                Tag::Offset(o) => offset = *o,
                Tag::Meta => {}
            }
        }
        if !stamps.is_empty() {
            lines.push((stamps, text.to_owned()));
        }
    }
    let mut out: Vec<LyricLine> = vec![];
    for (stamps, raw_text) in lines {
        let (text, words) = split_words(&raw_text);
        let single = stamps.len() == 1;
        for ms in stamps {
            let words = if single {
                words
                    .iter()
                    .map(|w| LyricWord {
                        start_ms: shift(w.start_ms, offset),
                        end_ms: if w.end_ms == 0 {
                            0
                        } else {
                            shift(w.end_ms, offset)
                        },
                        text: w.text.clone(),
                    })
                    .collect()
            } else {
                vec![]
            };
            out.push(LyricLine {
                start_ms: shift(ms, offset),
                text: text.clone(),
                words,
            });
        }
    }
    out.sort_by_key(|l| l.start_ms);
    let starts: Vec<u64> = out.iter().map(|l| l.start_ms).collect();
    for (i, line) in out.iter_mut().enumerate() {
        let line_end = starts.get(i + 1).copied().unwrap_or(line.start_ms + 5000);
        if let Some(last) = line.words.last_mut().filter(|w| w.end_ms <= w.start_ms) {
            last.end_ms = line_end.max(last.start_ms);
        }
    }
    out
}

/// Parse lyrics text of either kind: synced LRC when it has timestamps, otherwise plain lines
/// (metadata tag lines removed). `provider` is left empty.
pub fn parse(src: &str) -> Lyrics {
    let synced = parse_lrc(src);
    if !synced.is_empty() {
        let plain = synced
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return Lyrics {
            plain,
            synced,
            provider: String::new(),
        };
    }
    let kept: Vec<&str> = src
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|l| !matches!(leading_tags(l), (t, "") if !t.is_empty()))
        .map(str::trim_end)
        .collect();
    Lyrics {
        plain: kept.join("\n").trim().to_owned(),
        synced: vec![],
        provider: String::new(),
    }
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("lyrics_tests.rs");
}
