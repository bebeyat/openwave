//! Bridge to the single hidden `<audio>` element (`assets/audio.js`).
//!
//! * Events (JS -> Rust) arrive as JSON objects and are parsed into [`AudioEvent`].
//! * Commands (Rust -> JS) are tiny one-shot `eval` calls (`window.owAudio.cmd({...})`).
//! * Every `load` carries a generation number; events echo it so late events of a replaced
//!   source can be dropped by the controller.

use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

/// The audio bridge script, injected once by [`run_events`].
pub const AUDIO_JS: &str = include_str!("../../assets/audio.js");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioState {
    Playing,
    Paused,
    Buffering,
    Ended,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum AudioEvent {
    Time {
        g: u64,
        pos: f64,
        dur: Option<f64>,
    },
    State {
        g: u64,
        s: AudioState,
    },
    Error {
        g: u64,
        #[serde(default)]
        code: u32,
    },
    Vol {
        v: f64,
        muted: bool,
    },
    DeviceLost,
    /// The sound effects could not start (Web Audio or the pitch worklet is unavailable).
    FxFailed,
}

impl AudioEvent {
    /// Generation the event belongs to (`None` for events that are not tied to a source).
    pub fn generation(&self) -> Option<u64> {
        match self {
            AudioEvent::Time { g, .. }
            | AudioEvent::State { g, .. }
            | AudioEvent::Error { g, .. } => Some(*g),
            AudioEvent::Vol { .. } | AudioEvent::DeviceLost | AudioEvent::FxFailed => None,
        }
    }
}

/// Parse one message from the bridge; unknown or malformed messages are ignored.
pub fn parse_event(value: serde_json::Value) -> Option<AudioEvent> {
    serde_json::from_value(value).ok()
}

/// Position after seeking by `delta` seconds, kept inside `0..=duration` (duration 0 = unknown).
pub fn seek_target(position: f64, delta: f64, duration: f64) -> f64 {
    let t = (position + delta).max(0.0);
    if duration > 0.0 {
        t.min(duration)
    } else {
        t
    }
}

/// Keep a volume inside `0.0..=1.0` (NaN becomes 0).
pub fn clamp_volume(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Send a raw command object to the audio page (used by the effects).
pub(super) fn send(value: serde_json::Value) {
    cmd(value);
}

fn cmd(value: serde_json::Value) {
    document::eval(&format!("window.owAudio && window.owAudio.cmd({value});"));
}

/// Start the bridge and forward its events to `handler` until the page goes away.
/// Call once, from a long-lived component (the player bar).
pub async fn run_events(mut handler: impl FnMut(AudioEvent)) {
    let mut eval = document::eval(&format!("{AUDIO_JS}\n{}", super::effects::SCRIPTS));
    cmd(json!({ "c": "hello" }));
    super::effects::push_current();
    while let Ok(value) = eval.recv::<serde_json::Value>().await {
        if let Some(event) = parse_event(value) {
            handler(event);
        }
    }
}

/// Load a new source. `start` is the position (seconds) to jump to once metadata is known.
pub fn load(generation: u64, url: &str, start: f64, autoplay: bool) {
    cmd(json!({ "c": "load", "g": generation, "url": url, "start": start, "play": autoplay }));
}

pub fn play() {
    cmd(json!({ "c": "play" }));
}

pub fn pause() {
    cmd(json!({ "c": "pause" }));
}

/// Unload the source (nothing left to play).
pub fn stop() {
    cmd(json!({ "c": "stop" }));
}

pub fn seek(seconds: f64) {
    cmd(json!({ "c": "seek", "pos": seconds.max(0.0) }));
}

pub fn set_volume(volume: f64) {
    cmd(json!({ "c": "volume", "v": clamp_volume(volume) }));
}

pub fn set_muted(muted: bool) {
    cmd(json!({ "c": "mute", "on": muted }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bridge_messages() {
        let ev = parse_event(json!({"t":"time","g":3,"pos":12.5,"dur":200.0}));
        assert_eq!(
            ev,
            Some(AudioEvent::Time {
                g: 3,
                pos: 12.5,
                dur: Some(200.0)
            })
        );
        let ev = parse_event(json!({"t":"time","g":3,"pos":0,"dur":null}));
        assert_eq!(ev.and_then(|e| e.generation()), Some(3));
        let ev = parse_event(json!({"t":"state","g":1,"s":"buffering"}));
        assert_eq!(
            ev,
            Some(AudioEvent::State {
                g: 1,
                s: AudioState::Buffering
            })
        );
        assert_eq!(
            parse_event(json!({"t":"vol","v":0.5,"muted":true})),
            Some(AudioEvent::Vol {
                v: 0.5,
                muted: true
            })
        );
        assert_eq!(
            parse_event(json!({"t":"device_lost","g":9})),
            Some(AudioEvent::DeviceLost)
        );
        assert_eq!(
            parse_event(json!({"t":"error","g":2})).and_then(|e| e.generation()),
            Some(2)
        );
        assert_eq!(
            parse_event(json!({"t":"fx_failed","g":9})),
            Some(AudioEvent::FxFailed)
        );
    }

    #[test]
    fn ignores_unknown_messages() {
        assert_eq!(parse_event(json!({"t":"nope"})), None);
        assert_eq!(parse_event(json!("text")), None);
        assert_eq!(parse_event(json!({"t":"state","g":1,"s":"weird"})), None);
    }

    #[test]
    fn seek_target_stays_in_range() {
        assert_eq!(seek_target(10.0, 5.0, 100.0), 15.0);
        assert_eq!(seek_target(2.0, -5.0, 100.0), 0.0);
        assert_eq!(seek_target(98.0, 5.0, 100.0), 100.0);
        assert_eq!(seek_target(98.0, 5.0, 0.0), 103.0);
    }

    #[test]
    fn volume_is_clamped() {
        assert_eq!(clamp_volume(1.7), 1.0);
        assert_eq!(clamp_volume(-1.0), 0.0);
        assert_eq!(clamp_volume(f64::NAN), 0.0);
        assert_eq!(clamp_volume(0.4), 0.4);
    }
}
