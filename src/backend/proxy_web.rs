//! Streaming a googlevideo URL through the proxy in bounded range chunks.
//!
//! The audio element asks for `Range: bytes=0-` (the whole file). For long tracks googlevideo
//! answers such an open-ended request slowly (measured: 0.4 to 1.9 s to the first 64 KiB, against
//! 55 ms for `bytes=0-262143`), which is how yt-dlp's 10 MB chunking avoids throttling too. So
//! the proxy asks upstream for a small first chunk, answers the page with the full range it asked
//! for, and fetches the remaining chunks one after another while the body is being read.
use crate::backend::{error::MediaError, media::Media, proxy::bounds};
use axum::{
    body::{Body, Bytes},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{stream::BoxStream, StreamExt};

/// First upstream request: small so playback starts at once.
pub(super) const FIRST_CHUNK: u64 = 512 * 1024;
/// Later requests.
pub(super) const NEXT_CHUNK: u64 = 4 * 1024 * 1024;

/// `bytes=a-` or `bytes=a-b` as (a, b). Suffix ranges and lists are not chunked.
pub(super) fn parse_start_range(range: &str) -> Option<(u64, Option<u64>)> {
    let (a, b) = range.strip_prefix("bytes=")?.split_once('-')?;
    let start = a.parse().ok()?;
    let end = if b.is_empty() {
        None
    } else {
        Some(b.parse().ok()?)
    };
    end.is_none_or(|e| e >= start).then_some((start, end))
}

/// The upstream `Range` value for a chunk starting at `start` that must not pass `last`.
pub(super) fn chunk_header(start: u64, last: Option<u64>, chunk: u64) -> String {
    let cap = start.saturating_add(chunk - 1);
    let end = last.map_or(cap, |l| l.min(cap));
    format!("bytes={start}-{end}")
}

/// `bytes a-b/total` as (a, b, total).
pub(super) fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let (span, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (a, b) = span.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?, total.parse().ok()?))
}

fn upstream_error(host: &str, e: reqwest::Error) -> MediaError {
    eprintln!("media proxy: upstream {host}: {}", e.without_url());
    MediaError::Network("upstream".into())
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default()
}

fn content_range(resp: &reqwest::Response) -> Option<(u64, u64, u64)> {
    resp.headers()
        .get("content-range")?
        .to_str()
        .ok()
        .and_then(parse_content_range)
}

/// Forward a successful `upstream` (status, the usual headers, body). An error answer is never
/// relayed (no upstream headers or body reach the page): 416 becomes a bare 416, anything else a
/// generic 502.
fn passthrough(upstream: reqwest::Response, method: &Method) -> Result<Response, MediaError> {
    if upstream.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        return Ok(StatusCode::RANGE_NOT_SATISFIABLE.into_response());
    }
    if !upstream.status().is_success() {
        return Err(MediaError::BadResponse);
    }
    let mut builder = Response::builder().status(upstream.status());
    for h in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
    ] {
        if let Some(v) = upstream.headers().get(h) {
            builder = builder.header(h, v);
        }
    }
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(upstream.bytes_stream())
    };
    builder.body(body).map_err(|_| MediaError::BadResponse)
}

pub(super) async fn serve_web(
    media: &Media,
    url: String,
    range: Option<&str>,
    method: &Method,
) -> Result<Response, MediaError> {
    if range.is_some_and(|r| bounds(Some(r), u64::MAX).is_err()) {
        return Ok(StatusCode::RANGE_NOT_SATISFIABLE.into_response());
    }
    let host = host_of(&url);
    let plan = range
        .filter(|_| method != Method::HEAD)
        .and_then(parse_start_range);
    let mut request = media.client.request(method.clone(), url.as_str());
    if let Some((start, end)) = plan {
        request = request.header("Range", chunk_header(start, end, FIRST_CHUNK));
    } else if let Some(r) = range {
        request = request.header("Range", r);
    }
    let upstream = request.send().await.map_err(|e| upstream_error(&host, e))?;
    if !upstream.status().is_success() {
        eprintln!("media proxy: upstream {host}: status {}", upstream.status());
    }
    let (Some((start, end)), Some((got_start, got_end, total))) = (plan, content_range(&upstream))
    else {
        return passthrough(upstream, method);
    };
    let last = end.map_or(total - 1, |e| e.min(total - 1));
    if upstream.status().as_u16() != 206 || got_start != start || got_end >= last {
        return passthrough(upstream, method);
    }
    let mime = upstream.headers().get("content-type").cloned();
    let state = Chunks {
        client: media.client.clone(),
        url,
        host,
        next: got_end + 1,
        last,
        body: Some(upstream.bytes_stream().boxed()),
    };
    let mut builder = Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header("accept-ranges", "bytes")
        .header("content-range", format!("bytes {start}-{last}/{total}"))
        .header("content-length", last - start + 1);
    if let Some(m) = mime {
        builder = builder.header("content-type", m);
    }
    builder
        .body(Body::from_stream(state.into_stream()))
        .map_err(|_| MediaError::BadResponse)
}

/// The remaining chunks of one response, fetched lazily.
struct Chunks {
    client: reqwest::Client,
    url: String,
    host: String,
    /// First byte not yet requested.
    next: u64,
    /// Last byte the page asked for.
    last: u64,
    body: Option<BoxStream<'static, reqwest::Result<Bytes>>>,
}

impl Chunks {
    fn into_stream(self) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> {
        futures_util::stream::unfold(self, |mut st| async move {
            loop {
                if let Some(body) = st.body.as_mut() {
                    match body.next().await {
                        Some(Ok(bytes)) => return Some((Ok(bytes), st)),
                        Some(Err(e)) => return Some((Err(st.fail(e)), st.finished())),
                        None => st.body = None,
                    }
                }
                if st.next > st.last {
                    return None;
                }
                if let Err(e) = st.fetch_next().await {
                    return Some((Err(e), st.finished()));
                }
            }
        })
    }

    fn fail(&self, e: reqwest::Error) -> std::io::Error {
        eprintln!("media proxy: upstream {}: {}", self.host, e.without_url());
        std::io::Error::other("upstream")
    }

    fn finished(mut self) -> Self {
        self.body = None;
        self.next = self.last + 1;
        self
    }

    async fn fetch_next(&mut self) -> Result<(), std::io::Error> {
        let header = chunk_header(self.next, Some(self.last), NEXT_CHUNK);
        let resp = self
            .client
            .get(self.url.as_str())
            .header("Range", header)
            .send()
            .await
            .map_err(|e| self.fail(e))?;
        match content_range(&resp) {
            Some((a, b, _)) if resp.status().as_u16() == 206 && a == self.next => {
                self.next = b + 1;
                self.body = Some(resp.bytes_stream().boxed());
                Ok(())
            }
            _ => {
                eprintln!(
                    "media proxy: upstream {}: chunk status {}",
                    self.host,
                    resp.status()
                );
                Err(std::io::Error::other("upstream"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_closed_ranges_are_chunked_others_are_not() {
        assert_eq!(parse_start_range("bytes=0-"), Some((0, None)));
        assert_eq!(parse_start_range("bytes=100-199"), Some((100, Some(199))));
        assert_eq!(parse_start_range("bytes=-500"), None);
        assert_eq!(parse_start_range("bytes=5-2"), None);
        assert_eq!(parse_start_range("bytes=1-2,4-5"), None);
        assert_eq!(parse_start_range("items=0-"), None);
    }

    #[test]
    fn chunk_header_is_bounded_by_chunk_and_by_the_requested_end() {
        assert_eq!(chunk_header(0, None, FIRST_CHUNK), "bytes=0-524287");
        assert_eq!(chunk_header(0, Some(99), FIRST_CHUNK), "bytes=0-99");
        assert_eq!(chunk_header(1000, Some(u64::MAX), 10), "bytes=1000-1009");
        assert_eq!(
            chunk_header(u64::MAX - 1, None, 10),
            "bytes=18446744073709551614-18446744073709551615"
        );
    }

    #[test]
    fn content_range_parsing() {
        assert_eq!(parse_content_range("bytes 0-9/100"), Some((0, 9, 100)));
        assert_eq!(parse_content_range("bytes */100"), None);
        assert_eq!(parse_content_range("bytes 0-9/*"), None);
        assert_eq!(parse_content_range("0-9/100"), None);
    }
}
