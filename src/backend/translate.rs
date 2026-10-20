//! Lyrics translation through Google's public `translate_a/single` endpoint. Only used when the
//! user turns "Translate" on in the lyrics pane; it sends the lyric lines and the target language.
use super::error::LyricsError;
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};

pub const BASE: &str = "https://translate.googleapis.com";
/// Longest text sent in one request, in characters.
const CHUNK_CHARS: usize = 4500;
const MAX_LINES: usize = 2000;

fn http() -> Result<&'static reqwest::Client, LyricsError> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| LyricsError::Network(e.clone()))
}

/// Google's code for a UI language (Chinese needs its script).
pub fn target_code(lang: &str) -> &str {
    if lang == "zh" {
        "zh-CN"
    } else {
        lang
    }
}

/// Indices of the non-empty lines, grouped so each group joined with `\n` stays within
/// `limit` characters (a single longer line gets a group of its own).
pub(crate) fn chunks(lines: &[String], limit: usize) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = vec![];
    let mut size = 0;
    for (i, line) in lines.iter().enumerate() {
        let n = line.trim().chars().count();
        if n == 0 {
            continue;
        }
        match out.last_mut() {
            Some(group) if size + 1 + n <= limit => {
                group.push(i);
                size += 1 + n;
            }
            _ => {
                out.push(vec![i]);
                size = n;
            }
        }
    }
    out
}

/// The translated text of a `translate_a/single` answer: `[[["dst","src",...], ...], ...]`.
pub(crate) fn parse(v: &Value) -> Option<String> {
    let parts = v.get(0)?.as_array()?;
    let text: String = parts.iter().filter_map(|p| p.get(0)?.as_str()).collect();
    (!text.is_empty()).then_some(text)
}

async fn translate_chunk(base: &str, text: &str, target: &str) -> Result<String, LyricsError> {
    let res = http()?
        .get(format!("{base}/translate_a/single"))
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target_code(target)),
            ("dt", "t"),
            ("q", text),
        ])
        .send()
        .await
        .map_err(|e| LyricsError::Network(e.without_url().to_string()))?;
    if !res.status().is_success() {
        return Err(LyricsError::Unavailable);
    }
    let v: Value = res.json().await.map_err(|_| LyricsError::Unavailable)?;
    parse(&v).ok_or(LyricsError::Unavailable)
}

/// Translate `lines` into `target` (a two-letter UI language). The answer has one entry per
/// line; empty lines stay empty.
pub async fn translate_from(
    base: &str,
    lines: &[String],
    target: &str,
) -> Result<Vec<String>, LyricsError> {
    if lines.len() > MAX_LINES
        || target.is_empty()
        || !target.bytes().all(|b| b.is_ascii_alphabetic())
    {
        return Err(LyricsError::InvalidInput);
    }
    let mut out = vec![String::new(); lines.len()];
    for group in chunks(lines, CHUNK_CHARS) {
        let text: Vec<&str> = group.iter().map(|&i| lines[i].trim()).collect();
        let translated = translate_chunk(base, &text.join("\n"), target).await?;
        let got: Vec<&str> = translated.split('\n').collect();
        // Lines merged or split by the service cannot be mapped back reliably.
        if got.len() != group.len() {
            return Err(LyricsError::Unavailable);
        }
        for (&i, t) in group.iter().zip(got) {
            out[i] = t.trim().to_owned();
        }
    }
    Ok(out)
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("translate_tests.rs");
}
