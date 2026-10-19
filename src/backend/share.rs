//! Share links: a playlist (or one song) becomes `https://openwave.pages.dev/#<id>`, a link of the
//! same length whatever the size of the list. The tracks are stored as a signed Nostr event
//! (NIP-78 app data) on a few public relays; `<id>` is derived from the event content, so a reader
//! can check that what a relay returned is exactly what was shared. No account: every share is
//! signed with a fresh throwaway key. Everything here is pure except [`publish`] and [`fetch`].

use std::io::Read;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine;
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use futures_util::{stream::FuturesUnordered, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{tungstenite::Message, Connector};

use super::error::ShareError;
use super::palette_net::allowed_image_url;
use crate::model::{Cover, Text, Track};

/// The page that opens a share (or shows it in a browser). Fixed: old links must keep working.
pub const BASE_URL: &str = "https://openwave.pages.dev/#";
pub const SCHEME: &str = "openwave://s/";
const RELAYS: [&str; 5] = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://nostr.mom",
    "wss://relay.nostr.net",
];
/// NIP-78 "application-specific data".
const KIND: u64 = 30078;
const D_PREFIX: &str = "ow:";
const ID_LEN: usize = 10;
const BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// Biggest relay messages are 1 MB; a list this big is ~5,000 songs.
const MAX_CONTENT: usize = 900_000;
const MAX_INFLATED: u64 = 16 * 1024 * 1024;
const MAX_TRACKS: usize = 10_000;
const MAX_TEXT: usize = 200;
const TIMEOUT: Duration = Duration::from_secs(8);

/// What a link holds once read and checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    pub name: String,
    /// Shared from a song's menu: open it by playing, not as a playlist.
    pub single: bool,
    pub tracks: Vec<Track>,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    v: u8,
    n: String,
    #[serde(default)]
    s: bool,
    t: Vec<Track>,
}

/// The share id inside a link: `https://openwave.pages.dev/#<id>` (scheme optional) or `openwave://s/<id>`.
pub fn parse_link(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let id = s
        .strip_prefix(SCHEME)
        .or_else(|| s.strip_prefix(BASE_URL))
        .or_else(|| s.strip_prefix(&BASE_URL["https://".len()..]))?;
    (id.len() == ID_LEN && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.to_owned())
}

pub fn link(id: &str) -> String {
    format!("{BASE_URL}{id}")
}

/// 10 base62 characters (~59 bits) of the SHA-256 of `content`.
pub fn id_of(content: &str) -> String {
    let hash = Sha256::digest(content.as_bytes());
    let mut n = u64::from_be_bytes(hash[..8].try_into().expect("8 bytes"));
    let mut out = [b'0'; ID_LEN];
    for c in out.iter_mut().rev() {
        *c = BASE62[(n % 62) as usize];
        n /= 62;
    }
    String::from_utf8(out.to_vec()).expect("ascii")
}

fn capped(s: &str) -> String {
    s.trim().chars().take(MAX_TEXT).collect()
}

fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A shared track is untrusted input: keep only catalog ids, short texts and allowed cover hosts.
fn clean(t: Track) -> Option<Track> {
    if !valid_id(&t.id) {
        return None;
    }
    let text = |s: &Text| Text::from(capped(s));
    Some(Track {
        title: text(&t.title),
        artists: t
            .artists
            .into_iter()
            .filter(|a| a.id.is_empty() || valid_id(&a.id))
            .take(20)
            .map(|a| crate::model::ArtistRef {
                name: text(&a.name),
                id: a.id,
            })
            .collect(),
        album: t
            .album
            .filter(|a| a.id.is_empty() || valid_id(&a.id))
            .map(|a| crate::model::AlbumRef {
                title: text(&a.title),
                id: a.id,
            }),
        cover: t
            .cover
            .filter(|c| allowed_image_url(&c.url))
            .map(|c| Cover {
                url: c.url,
                width: c.width.min(4096),
                height: c.height.min(4096),
            }),
        ..t
    })
}

/// Deflate + base64url of the list. Local files are left out (they exist only on this computer).
pub fn encode(name: &str, tracks: &[Track], single: bool) -> Result<String, ShareError> {
    use std::io::Write;
    let tracks: Vec<Track> = tracks.iter().filter(|t| !t.is_local()).cloned().collect();
    if tracks.is_empty() {
        return Err(ShareError::Nothing);
    }
    let payload = Payload {
        v: 1,
        n: capped(name),
        s: single,
        t: tracks,
    };
    let json = serde_json::to_vec(&payload).map_err(|_| ShareError::Nothing)?;
    let mut z = DeflateEncoder::new(Vec::new(), Compression::best());
    z.write_all(&json).map_err(|_| ShareError::Nothing)?;
    let bytes = z.finish().map_err(|_| ShareError::Nothing)?;
    let content = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    if content.len() > MAX_CONTENT {
        return Err(ShareError::TooLarge);
    }
    Ok(content)
}

/// The list in `content`, but only when `content` really is the share `id`.
pub fn decode(id: &str, content: &str) -> Option<Shared> {
    if content.len() > MAX_CONTENT || id_of(content) != id {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(content)
        .ok()?;
    let mut json = Vec::new();
    DeflateDecoder::new(&bytes[..])
        .take(MAX_INFLATED)
        .read_to_end(&mut json)
        .ok()?;
    let p: Payload = serde_json::from_slice(&json).ok()?;
    let tracks: Vec<Track> = p.t.into_iter().take(MAX_TRACKS).filter_map(clean).collect();
    (p.v == 1 && !tracks.is_empty()).then(|| Shared {
        name: capped(&p.n),
        single: p.s,
        tracks,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn random32() -> Result<[u8; 32], ShareError> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).map_err(|_| ShareError::Offline)?;
    Ok(b)
}

/// A NIP-01 event signed with a fresh key (BIP-340 Schnorr over the event id).
pub fn signed_event(content: &str, created_at: u64) -> Result<Value, ShareError> {
    let key = loop {
        // Fails only for the ~2^-128 chance of an out-of-range scalar.
        if let Ok(k) = k256::schnorr::SigningKey::from_bytes(&random32()?) {
            break k;
        }
    };
    let pubkey = hex(&key.verifying_key().to_bytes());
    let tags = json!([
        ["d", format!("{D_PREFIX}{}", id_of(content))],
        ["alt", "OpenWave shared playlist"]
    ]);
    let serial = serde_json::to_string(&json!([0, pubkey, created_at, KIND, tags, content]))
        .map_err(|_| ShareError::Nothing)?;
    let id: [u8; 32] = Sha256::digest(serial.as_bytes()).into();
    let sig = key
        .sign_raw(&id, &random32()?)
        .map_err(|_| ShareError::Offline)?;
    Ok(json!({
        "id": hex(&id),
        "pubkey": pubkey,
        "created_at": created_at,
        "kind": KIND,
        "tags": tags,
        "content": content,
        "sig": hex(&sig.to_bytes()),
    }))
}

/// rustls has two crypto providers compiled in (through reqwest), so pick one explicitly.
fn connector() -> Connector {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    let config = CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        Arc::new(config)
    });
    Connector::Rustls(config.clone())
}

/// Send `msg` to `relay` and hand every text frame to `on_frame` until it returns `Some`.
async fn talk<T>(
    relay: &str,
    msg: String,
    mut on_frame: impl FnMut(Value) -> Option<T>,
) -> Option<T> {
    let run = async {
        let (mut ws, _) =
            tokio_tungstenite::connect_async_tls_with_config(relay, None, false, Some(connector()))
                .await
                .ok()?;
        ws.send(Message::text(msg)).await.ok()?;
        while let Some(Ok(frame)) = ws.next().await {
            let Message::Text(text) = frame else { continue };
            let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else {
                continue;
            };
            if let Some(out) = on_frame(v) {
                let _ = ws.close(None).await;
                return Some(out);
            }
        }
        None
    };
    tokio::time::timeout(TIMEOUT, run).await.ok().flatten()
}

/// Store the list on the relays and return its link. Succeeds when at least one relay keeps it.
pub async fn publish(name: &str, tracks: &[Track], single: bool) -> Result<String, ShareError> {
    let content = encode(name, tracks, single)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let event = signed_event(&content, now)?;
    let event_id = event["id"].as_str().unwrap_or_default().to_owned();
    let msg = json!(["EVENT", event]).to_string();
    let mut sends: FuturesUnordered<_> = RELAYS
        .iter()
        .map(|r| {
            talk(r, msg.clone(), |v| {
                (v[0] == "OK" && v[1] == event_id.as_str()).then(|| v[2] == true)
            })
        })
        .collect();
    let mut stored = false;
    while let Some(ok) = sends.next().await {
        stored |= ok == Some(true);
    }
    if stored {
        Ok(link(&id_of(&content)))
    } else {
        Err(ShareError::Offline)
    }
}

enum Lookup {
    Found(Shared),
    Missing,
}

async fn lookup(relay: &str, id: &str) -> Option<Lookup> {
    let req =
        json!(["REQ", "ow", {"kinds": [KIND], "#d": [format!("{D_PREFIX}{id}")], "limit": 5}]);
    talk(relay, req.to_string(), |v| match v[0].as_str() {
        // Anyone can post under any d tag; only content that hashes to the id is accepted.
        Some("EVENT") => v[2]["content"]
            .as_str()
            .and_then(|c| decode(id, c))
            .map(Lookup::Found),
        Some("EOSE" | "CLOSED") => Some(Lookup::Missing),
        _ => None,
    })
    .await
}

/// Read a share from whichever relay answers first.
pub async fn fetch(id: &str) -> Result<Shared, ShareError> {
    let mut asks: FuturesUnordered<_> = RELAYS.iter().map(|r| lookup(r, id)).collect();
    let mut reached = false;
    while let Some(answer) = asks.next().await {
        match answer {
            Some(Lookup::Found(s)) => return Ok(s),
            Some(Lookup::Missing) => reached = true,
            None => {}
        }
    }
    Err(if reached {
        ShareError::NotFound
    } else {
        ShareError::Offline
    })
}

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
