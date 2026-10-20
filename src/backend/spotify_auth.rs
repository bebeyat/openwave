//! Signing in to Spotify with the user's own app (Client ID from the Spotify developer
//! dashboard): OAuth authorization code with PKCE, a one-shot loopback listener for the redirect,
//! and the tokens kept in `<data>/spotify-auth.json`. Nothing here runs until the user presses
//! "Sign in"; "Sign out" deletes the file.
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub const ACCOUNTS: &str = "https://accounts.spotify.com";
pub const FILE: &str = "spotify-auth.json";
/// Fixed so the redirect URI can be registered in the Spotify dashboard.
pub const REDIRECT_PORT: u16 = 43821;
const SCOPES: &str = "playlist-read-private user-library-read";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
/// A token this close to its expiry is refreshed first.
const EXPIRY_MARGIN_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    NoClientId,
    PortBusy,
    Denied,
    Timeout,
    Network,
    BadResponse,
    /// Not signed in (or the saved sign-in no longer works).
    SignedOut,
}

impl AuthError {
    /// i18n key of the message.
    pub fn key(self) -> &'static str {
        match self {
            Self::NoClientId => "spotify.error.client_id",
            Self::PortBusy => "spotify.error.port",
            Self::Denied => "spotify.error.denied",
            Self::Timeout => "spotify.error.timeout",
            Self::Network => "error.network",
            Self::BadResponse | Self::SignedOut => "spotify.error.failed",
        }
    }
}

/// The saved sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: u64,
}

pub fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}/callback")
}

/// Spotify client ids are 32 hex digits.
pub fn valid_client_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn random_text(bytes: usize) -> Result<String, AuthError> {
    let mut b = vec![0u8; bytes];
    getrandom::getrandom(&mut b).map_err(|_| AuthError::BadResponse)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

/// PKCE S256 challenge of `verifier`.
pub(crate) fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(crate) fn authorize_url(
    base: &str,
    client_id: &str,
    challenge: &str,
    state: &str,
) -> Option<String> {
    let mut url = reqwest::Url::parse(&format!("{base}/authorize")).ok()?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri())
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge)
        .append_pair("state", state)
        .append_pair("scope", SCOPES);
    Some(url.to_string())
}

/// The authorization code of a redirect, only when its `state` is ours.
pub(crate) fn code_from(query: &HashMap<String, String>, state: &str) -> Result<String, AuthError> {
    if query.get("state").map(String::as_str) != Some(state) {
        return Err(AuthError::BadResponse);
    }
    if query.contains_key("error") {
        return Err(AuthError::Denied);
    }
    query
        .get("code")
        .filter(|c| !c.is_empty() && c.len() <= 2048)
        .cloned()
        .ok_or(AuthError::BadResponse)
}

fn http() -> Result<reqwest::Client, AuthError> {
    reqwest::Client::builder()
        .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AuthError::Network)
}

/// Serve `/callback` once on `listener` and return the code it brought.
async fn wait_for_code(
    listener: tokio::net::TcpListener,
    state: String,
) -> Result<String, AuthError> {
    use axum::{extract::Query, response::Html, routing::get, Router};
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, AuthError>>(1);
    let app = Router::new().route(
        "/callback",
        get(move |Query(q): Query<HashMap<String, String>>| {
            let (tx, state) = (tx.clone(), state.clone());
            async move {
                let _ = tx.try_send(code_from(&q, &state));
                Html("<!doctype html><meta charset=utf-8><title>OpenWave</title><p>OpenWave: ✓</p>")
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await });
    let mut rx = rx;
    let got = tokio::time::timeout(SIGN_IN_TIMEOUT, rx.recv()).await;
    server.abort();
    match got {
        Ok(Some(r)) => r,
        _ => Err(AuthError::Timeout),
    }
}

async fn token_request(base: &str, form: &[(&str, &str)]) -> Result<serde_json::Value, AuthError> {
    let res = http()?
        .post(format!("{base}/api/token"))
        .form(form)
        .send()
        .await
        .map_err(|_| AuthError::Network)?;
    if res.status().as_u16() == 400 {
        return Err(AuthError::SignedOut);
    }
    if !res.status().is_success() {
        return Err(AuthError::BadResponse);
    }
    res.json().await.map_err(|_| AuthError::BadResponse)
}

fn tokens_from(
    v: &serde_json::Value,
    client_id: &str,
    old_refresh: Option<&str>,
) -> Option<Tokens> {
    let refresh = v["refresh_token"].as_str().or(old_refresh)?;
    Some(Tokens {
        client_id: client_id.to_owned(),
        access_token: v["access_token"].as_str()?.to_owned(),
        refresh_token: refresh.to_owned(),
        expires_at_ms: now_ms() + v["expires_in"].as_u64().unwrap_or(3600) * 1000,
    })
}

/// The whole sign-in: listen on the loopback port, hand the authorize URL to `open` (the
/// browser), wait for the redirect, exchange the code.
pub(crate) async fn sign_in_with(
    base: &str,
    listener: tokio::net::TcpListener,
    client_id: &str,
    open: impl FnOnce(&str),
) -> Result<Tokens, AuthError> {
    if !valid_client_id(client_id) {
        return Err(AuthError::NoClientId);
    }
    let verifier = random_text(64)?;
    let state = random_text(16)?;
    let url = authorize_url(base, client_id, &challenge(&verifier), &state)
        .ok_or(AuthError::BadResponse)?;
    open(&url);
    let code = wait_for_code(listener, state).await?;
    let redirect = redirect_uri();
    let v = token_request(
        base,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("client_id", client_id),
            ("code_verifier", &verifier),
        ],
    )
    .await?;
    tokens_from(&v, client_id, None).ok_or(AuthError::BadResponse)
}

/// Sign in with `client_id` and save the tokens in `dir`.
pub async fn sign_in(dir: &Path, client_id: &str) -> Result<(), AuthError> {
    if !valid_client_id(client_id) {
        return Err(AuthError::NoClientId);
    }
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, REDIRECT_PORT))
        .await
        .map_err(|_| AuthError::PortBusy)?;
    let tokens = sign_in_with(ACCOUNTS, listener, client_id, |url| {
        let _ = open::that(url);
    })
    .await?;
    save(dir, &tokens)
}

pub fn load(dir: &Path) -> Option<Tokens> {
    serde_json::from_slice(&std::fs::read(dir.join(FILE)).ok()?).ok()
}

fn save(dir: &Path, tokens: &Tokens) -> Result<(), AuthError> {
    let bytes = serde_json::to_vec(tokens).map_err(|_| AuthError::BadResponse)?;
    let tmp = dir.join(format!("{FILE}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, dir.join(FILE)))
        .map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            AuthError::BadResponse
        })
}

pub fn sign_out(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(dir.join(FILE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// A usable access token, refreshed (and saved) when it is about to expire.
pub async fn access_token(dir: &Path, base: &str) -> Result<String, AuthError> {
    let saved = load(dir).ok_or(AuthError::SignedOut)?;
    if saved.expires_at_ms > now_ms() + EXPIRY_MARGIN_MS {
        return Ok(saved.access_token);
    }
    let v = token_request(
        base,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &saved.refresh_token),
            ("client_id", &saved.client_id),
        ],
    )
    .await?;
    let fresh = tokens_from(&v, &saved.client_id, Some(&saved.refresh_token))
        .ok_or(AuthError::BadResponse)?;
    save(dir, &fresh)?;
    Ok(fresh.access_token)
}

#[cfg(all(test, private_tests))]
mod tests {
    include!("spotify_auth_tests.rs");
}
