//! Loopback streaming proxy used by the hidden `<audio>` element.
//!
//! Bound to 127.0.0.1 on an ephemeral port; every URL is `/<per-launch token>/<ticket>`, so other
//! local processes and web pages cannot read audio without knowing the token. Upstream requests are
//! limited to googlevideo hosts (see `media::stream_url`); local files are served from the exact
//! paths the user picked.
//!
//! Hardening: the `Host` header must name this exact loopback address (blocks DNS-rebinding), error
//! bodies are generic (upstream addresses are signed and must not leak into pages or logs), every
//! response is `nosniff`. CORS is narrow: the audio element is routed through Web Audio for the
//! sound effects, which needs a CORS-enabled media load (`crossorigin="anonymous"`), so an
//! authorized response echoes `Access-Control-Allow-Origin` for exactly the web view's own
//! origin ([`WEBVIEW_ORIGIN`]) and for no other origin (never `*`).
use super::{
    error::{Coded, MediaError},
    media::{validate_local, Media},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::{path::PathBuf, sync::Arc, time::Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[path = "proxy_web.rs"]
mod web;

#[derive(Clone)]
pub(super) enum Input {
    Web(String),
    /// A file in our own cache folder (trusted path).
    File(PathBuf),
    /// A user-added music file: re-validated every time it is served.
    Local(PathBuf),
}

#[derive(Clone)]
pub(super) struct Ticket {
    pub input: Input,
    pub mime: String,
    /// Last time the ticket was used; it expires after `TICKET_IDLE_TTL` without use.
    pub used: Instant,
}

/// Origin of the app's own web view (what wry/WebView2 reports as `location.origin` on Windows).
/// The only origin that may read proxy responses.
pub(super) const WEBVIEW_ORIGIN: &str = crate::app::WEBVIEW_ORIGIN;

/// `Some(origin)` when the request's `Origin` header is exactly [`WEBVIEW_ORIGIN`].
pub(super) fn allowed_origin(headers: &HeaderMap) -> Option<HeaderValue> {
    headers
        .get(header::ORIGIN)
        .filter(|v| v.as_bytes() == WEBVIEW_ORIGIN.as_bytes())
        .cloned()
}

/// Only `127.0.0.1:<port>` and `localhost:<port>` may be named in the `Host` header.
pub(super) fn host_allowed(host: Option<&str>, port: u16) -> bool {
    host.is_some_and(|h| {
        let h = h.to_ascii_lowercase();
        h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}")
    })
}

/// Compare two secrets without stopping at the first differing byte.
pub(super) fn secret_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

pub(super) fn router(state: Arc<Media>) -> Router {
    Router::new()
        .route("/{secret}/{ticket}", get(read))
        .with_state(state)
}

/// Parse a single `Range: bytes=a-b` header against `size` into (start, end, is_partial).
pub(super) fn bounds(range: Option<&str>, size: u64) -> Result<(u64, u64, bool), ()> {
    if size == 0 {
        return Err(());
    }
    let Some(range) = range else {
        return Ok((0, size - 1, false));
    };
    let (start, end) = range
        .strip_prefix("bytes=")
        .ok_or(())?
        .split_once('-')
        .ok_or(())?;
    if start.is_empty() {
        let tail = end.parse::<u64>().map_err(|_| ())?;
        if tail == 0 {
            return Err(());
        }
        return Ok((size.saturating_sub(tail), size - 1, true));
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if start > end || start >= size {
        return Err(());
    }
    Ok((start, end, true))
}

fn local_mime<'a>(path: &std::path::Path, fallback: &'a str) -> &'a str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("mp3") => "audio/mpeg",
        Some("aac") => "audio/aac",
        Some("flac") => "audio/flac",
        Some("ogg" | "opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("m4a" | "mp4") => "audio/mp4",
        _ => fallback,
    }
}

async fn read(
    State(media): State<Arc<Media>>,
    Path((secret, key)): Path<(String, String)>,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let authorized = host_allowed(host, media.port) && secret_eq(&secret, &media.secret);
    let mut response = if !authorized {
        StatusCode::FORBIDDEN.into_response()
    } else {
        match serve(&media, &key, &headers, &method).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("media proxy: request failed ({})", e.code());
                (StatusCode::BAD_GATEWAY, "unavailable").into_response()
            }
        }
    };
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if let Some(origin) = allowed_origin(&headers).filter(|_| authorized) {
        let out = response.headers_mut();
        out.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        out.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    response
}

async fn serve(
    media: &Media,
    key: &str,
    headers: &HeaderMap,
    method: &Method,
) -> Result<Response, MediaError> {
    // Covers of local files: only `cover-<16 hex>` from our own covers folder.
    if let Some(hash) = key.strip_prefix("cover-") {
        if !crate::backend::local_tags::valid_cover_hash(hash) {
            return Err(MediaError::InvalidTrackId);
        }
        let path = media.covers_dir().join(format!("{hash}.jpg"));
        if !tokio::fs::metadata(&path).await.is_ok_and(|m| m.is_file()) {
            return Ok(StatusCode::NOT_FOUND.into_response());
        }
        let mut response = serve_file(&path, "image/jpeg", None, method).await?;
        // Content-addressed: the same name always holds the same picture.
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=31536000, immutable"),
        );
        return Ok(response);
    }
    let ticket = media
        .touch_ticket(key, Instant::now())
        .ok_or(MediaError::SessionExpired)?;
    let range = headers.get("Range").and_then(|v| v.to_str().ok());
    match ticket.input {
        Input::Web(url) => web::serve_web(media, url, range, method).await,
        Input::File(path) => serve_file(&path, &ticket.mime, range, method).await,
        Input::Local(path) => {
            let path = validate_local(&path).await?;
            serve_file(&path, &ticket.mime, range, method).await
        }
    }
}

async fn serve_file(
    path: &std::path::Path,
    mime: &str,
    range: Option<&str>,
    method: &Method,
) -> Result<Response, MediaError> {
    let mut file = tokio::fs::File::open(path).await?;
    let size = file.metadata().await?.len();
    let Ok((start, end, partial)) = bounds(range, size) else {
        return Ok((
            StatusCode::RANGE_NOT_SATISFIABLE,
            [("Content-Range", format!("bytes */{size}"))],
        )
            .into_response());
    };
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut builder = Response::builder()
        .status(if partial { 206 } else { 200 })
        .header("Content-Type", local_mime(path, mime))
        .header("Accept-Ranges", "bytes")
        .header("Content-Length", end - start + 1);
    if partial {
        builder = builder.header("Content-Range", format!("bytes {start}-{end}/{size}"));
    }
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(tokio_util::io::ReaderStream::new(
            file.take(end - start + 1),
        ))
    };
    builder.body(body).map_err(|_| MediaError::BadResponse)
}
