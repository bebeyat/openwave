//! Listen together: a host shares what plays through the same public Nostr relays as share links,
//! and guests follow. No OpenWave server: messages are *ephemeral* events (NIP-16 kinds
//! 20000-29999), which relays pass on to whoever is subscribed and never store.
//!
//! The room code is a hash of the host's session public key, so a guest accepts playback state
//! only from the key that made the code; anyone in the room may suggest songs (`Add`), and the
//! host adds them to its queue. Everything here is pure except [`connect`].

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use super::error::ShareError;
use super::share::{self, connector, event_id, hex, id_of, RELAYS};
use crate::model::Track;

pub use k256::schnorr::SigningKey;

/// The page that opens a session invite. Distinct from share links (`#<10 chars>`).
pub const LINK_PREFIX: &str = "https://openwave.pages.dev/#j/";
pub const SCHEME: &str = "openwave://j/";
/// Ephemeral: relays forward it and never keep it.
const KIND: u64 = 21078;
const CODE_LEN: usize = 10;
/// Biggest message accepted from a relay (a state with a full upcoming list is ~30 KB).
const MAX_CONTENT: usize = 200_000;
/// Songs sent with a state (the playing one first) and in one suggestion.
pub const MAX_QUEUE: usize = 50;
const MAX_ADD: usize = 50;
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
pub enum Msg {
    /// Host: what plays. `seq` changes on every host action (song, play/pause, seek), not on the
    /// periodic repeats; `at` is the host clock (ms) when `pos` (s) was read; `q[0]` plays.
    State {
        seq: u64,
        at: u64,
        pos: f64,
        play: bool,
        q: Vec<Track>,
    },
    /// Guest: songs to add to the host's queue.
    Add { t: Vec<Track> },
    /// Host: the session is over.
    End,
}

/// A message read from a relay, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    pub id: String,
    pub msg: Msg,
}

/// The room code of a host key: 10 base62 characters of the hash of its public key.
pub fn code_of(key: &SigningKey) -> String {
    id_of(&hex(&key.verifying_key().to_bytes()))
}

pub fn new_key() -> Result<SigningKey, ShareError> {
    share::new_key()
}

pub fn link(code: &str) -> String {
    format!("{LINK_PREFIX}{code}")
}

/// The code in an invite link: `https://openwave.pages.dev/#j/<code>` (scheme optional) or
/// `openwave://j/<code>`.
pub fn parse_join(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/');
    let code = s
        .strip_prefix(SCHEME)
        .or_else(|| s.strip_prefix(LINK_PREFIX))
        .or_else(|| s.strip_prefix(&LINK_PREFIX["https://".len()..]))?;
    valid_code(code).then(|| code.to_owned())
}

/// What someone typed to join: an invite link or the bare code.
pub fn parse_code(input: &str) -> Option<String> {
    let s = input.trim();
    parse_join(s).or_else(|| valid_code(s).then(|| s.to_owned()))
}

fn valid_code(code: &str) -> bool {
    code.len() == CODE_LEN && code.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The relay tag of a room. Hashed so the code itself never appears on the relays.
fn topic(code: &str) -> String {
    format!("owlt:{}", id_of(&format!("room:{code}")))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The relay message that sends `msg` to the room, signed with `key`.
pub fn outgoing(key: &SigningKey, code: &str, msg: &Msg) -> Result<String, ShareError> {
    let content = serde_json::to_string(msg).map_err(|_| ShareError::Nothing)?;
    let tags = json!([["t", topic(code)]]);
    let event = share::sign(key, KIND, tags, &content, now_secs())?;
    Ok(json!(["EVENT", event]).to_string())
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    s.len()
        .is_multiple_of(2)
        .then(|| {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
                .collect()
        })
        .flatten()
}

/// The author's public key (hex) when `event` is correctly signed.
fn verified_author(event: &Value) -> Option<&str> {
    let pubkey = event["pubkey"].as_str()?;
    let content = event["content"].as_str()?;
    let id = event_id(
        pubkey,
        event["created_at"].as_u64()?,
        event["kind"].as_u64()?,
        &event["tags"],
        content,
    );
    if event["id"].as_str()? != hex(&id) {
        return None;
    }
    let key = k256::schnorr::VerifyingKey::from_slice(&unhex(pubkey)?).ok()?;
    let sig = k256::schnorr::Signature::try_from(unhex(event["sig"].as_str()?)?.as_slice()).ok()?;
    key.verify_raw(&id, &sig).ok()?;
    Some(pubkey)
}

/// Shared tracks are untrusted: catalog songs only, cleaned like share links.
fn cleaned(tracks: Vec<Track>, max: usize) -> Vec<Track> {
    tracks
        .into_iter()
        .take(max)
        .filter(|t| !t.is_local())
        .filter_map(share::clean)
        .collect()
}

/// Check one event from a relay: right kind and room, valid signature, and state or end only
/// from the host whose key made `code`.
pub fn read(code: &str, event: &Value) -> Option<Incoming> {
    if event["kind"].as_u64()? != KIND
        || event["content"].as_str()?.len() > MAX_CONTENT
        || !event["tags"]
            .as_array()?
            .iter()
            .any(|t| t[0] == "t" && t[1] == topic(code).as_str())
    {
        return None;
    }
    let author = verified_author(event)?;
    let from_host = id_of(author) == code;
    let msg = match serde_json::from_str(event["content"].as_str()?).ok()? {
        Msg::State {
            seq,
            at,
            pos,
            play,
            q,
        } if from_host && pos.is_finite() => Msg::State {
            seq,
            at,
            pos: pos.max(0.0),
            play,
            q: cleaned(q, MAX_QUEUE),
        },
        Msg::End if from_host => Msg::End,
        Msg::Add { t } => Msg::Add {
            t: cleaned(t, MAX_ADD),
        },
        _ => return None,
    };
    Some(Incoming {
        id: event["id"].as_str()?.to_owned(),
        msg,
    })
}

/// An open room: send relay messages with `send`, read the room's events from `events`.
/// Dropping it closes every relay connection.
pub struct Connection {
    pub send: broadcast::Sender<String>,
    pub events: mpsc::UnboundedReceiver<Value>,
    stop: CancellationToken,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

/// Subscribe to the room on every relay, on the runtime `rt`. Connections that drop are retried.
pub fn connect(rt: &tokio::runtime::Handle, code: &str) -> Connection {
    let (send, _) = broadcast::channel(32);
    let (tx, events) = mpsc::unbounded_channel();
    let stop = CancellationToken::new();
    let req = json!(["REQ", "owlt", {"kinds": [KIND], "#t": [topic(code)], "since": now_secs()}])
        .to_string();
    for relay in RELAYS {
        let (out, tx, stop, req) = (send.subscribe(), tx.clone(), stop.clone(), req.clone());
        rt.spawn(async move {
            tokio::select! {
                _ = stop.cancelled() => {}
                _ = keep_open(relay, req, out, tx) => {}
            }
        });
    }
    Connection { send, events, stop }
}

async fn keep_open(
    relay: &str,
    req: String,
    mut out: broadcast::Receiver<String>,
    tx: mpsc::UnboundedSender<Value>,
) {
    let mut wait = RETRY_MIN;
    loop {
        if session(relay, &req, &mut out, &tx).await {
            wait = RETRY_MIN;
        }
        if tx.is_closed() {
            return;
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(RETRY_MAX);
    }
}

/// One connection to `relay`, until it fails. Returns whether it got as far as subscribing.
async fn session(
    relay: &str,
    req: &str,
    out: &mut broadcast::Receiver<String>,
    tx: &mpsc::UnboundedSender<Value>,
) -> bool {
    let Ok(Ok((mut ws, _))) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async_tls_with_config(relay, None, false, Some(connector())),
    )
    .await
    else {
        return false;
    };
    if ws.send(Message::text(req)).await.is_err() {
        return false;
    }
    loop {
        tokio::select! {
            msg = out.recv() => match msg {
                Ok(text) => if ws.send(Message::text(text)).await.is_err() { return true },
                // Fell behind: the next message carries the full state anyway.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return true,
            },
            frame = ws.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(v) = serde_json::from_str::<Value>(text.as_str()) {
                        if v[0] == "EVENT" && tx.send(v[2].clone()).is_err() {
                            return true;
                        }
                    }
                }
                Some(Ok(_)) => {}
                _ => return true,
            },
        }
    }
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("listen_tests.rs");
}
