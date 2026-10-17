//! Pure mapping from what is playing to the Discord "Listening to" activity. No I/O.

/// What the player is doing, as far as Discord cares.
#[derive(Clone, Debug, PartialEq)]
pub struct NowPlaying {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover: Option<String>,
    pub duration_s: f64,
    pub position_s: f64,
    pub playing: bool,
}

/// The activity fields we send. Times are Unix milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresenceView {
    pub details: String,
    pub state: String,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub button_url: Option<String>,
}

/// Discord rejects strings shorter than 2 or longer than 128 characters.
fn field(s: &str, fallback: &str) -> String {
    let t = s.trim();
    let t = if t.chars().count() < 2 { fallback } else { t };
    let mut out: String = t.chars().take(128).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}

/// Only plain HTTPS image URLs Discord can proxy, within its 256-character limit.
fn image_url(url: &str) -> Option<String> {
    (url.starts_with("https://") && url.len() <= 256 && !url.contains(char::is_whitespace))
        .then(|| url.to_owned())
}

/// Catalog tracks get a "listen" link; local files (and anything not an 11-char video id) do not.
fn listen_url(id: &str) -> Option<String> {
    let video = id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    video.then(|| format!("https://music.youtube.com/watch?v={id}"))
}

/// The activity for `np` at `now_ms`, or `None` to clear it (paused or stopped).
pub fn view(np: &NowPlaying, now_ms: i64) -> Option<PresenceView> {
    if !np.playing {
        return None;
    }
    let position_ms = (np.position_s.max(0.0) * 1000.0).round() as i64;
    let start = now_ms - position_ms;
    let end = (np.duration_s.is_finite() && np.duration_s > 1.0)
        .then(|| start + (np.duration_s * 1000.0).round() as i64);
    Some(PresenceView {
        details: field(&np.title, "OpenWave"),
        state: field(&np.artist, "OpenWave"),
        large_image: np.cover.as_deref().and_then(image_url),
        large_text: np
            .album
            .as_deref()
            .filter(|a| a.trim().chars().count() >= 2)
            .map(|a| field(a, "OpenWave")),
        start_ms: Some(start),
        end_ms: end,
        button_url: listen_url(&np.id),
    })
}

/// True when `next` differs enough from `last` to be worth sending: another song, play/pause,
/// or a seek (start time moved by more than two seconds).
pub fn worth_sending(last: Option<&PresenceView>, next: Option<&PresenceView>) -> bool {
    match (last, next) {
        (None, None) => false,
        (Some(a), Some(b)) => {
            let moved = match (a.start_ms, b.start_ms) {
                (Some(x), Some(y)) => (x - y).abs() > 2_000,
                _ => true,
            };
            a.details != b.details || a.state != b.state || a.large_image != b.large_image || moved
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn np() -> NowPlaying {
        NowPlaying {
            id: "dQw4w9WgXcQ".into(),
            title: "Kuzu Kuzu".into(),
            artist: "Tarkan".into(),
            album: Some("Karma".into()),
            cover: Some("https://lh3.googleusercontent.com/x=w544".into()),
            duration_s: 200.0,
            position_s: 30.0,
            playing: true,
        }
    }

    #[test]
    fn playing_track_maps_to_listening_activity_with_a_live_timeline() {
        let v = view(&np(), 1_000_000).unwrap();
        assert_eq!(
            (v.details.as_str(), v.state.as_str()),
            ("Kuzu Kuzu", "Tarkan")
        );
        assert_eq!(v.start_ms, Some(1_000_000 - 30_000));
        assert_eq!(v.end_ms, Some(1_000_000 - 30_000 + 200_000));
        assert_eq!(v.large_text.as_deref(), Some("Karma"));
        assert_eq!(
            v.button_url.as_deref(),
            Some("https://music.youtube.com/watch?v=dQw4w9WgXcQ")
        );
    }

    #[test]
    fn paused_clears_and_bad_fields_are_made_safe() {
        assert!(view(
            &NowPlaying {
                playing: false,
                ..np()
            },
            0
        )
        .is_none());
        let odd = NowPlaying {
            id: "local-1234".into(),
            title: "X".into(),
            artist: "".into(),
            album: Some(" ".into()),
            cover: Some("http://insecure/a.jpg".into()),
            duration_s: f64::NAN,
            ..np()
        };
        let v = view(&odd, 0).unwrap();
        assert_eq!(
            (v.details.as_str(), v.state.as_str()),
            ("OpenWave", "OpenWave")
        );
        assert_eq!(
            (v.large_image, v.large_text, v.end_ms, v.button_url),
            (None, None, None, None)
        );
        let long = NowPlaying {
            title: "a".repeat(300),
            ..np()
        };
        assert_eq!(view(&long, 0).unwrap().details.chars().count(), 128);
    }

    #[test]
    fn only_real_changes_are_sent() {
        let a = view(&np(), 1_000_000);
        let tick = view(
            &NowPlaying {
                position_s: 31.0,
                ..np()
            },
            1_001_000,
        );
        let seek = view(
            &NowPlaying {
                position_s: 90.0,
                ..np()
            },
            1_001_000,
        );
        let other = view(
            &NowPlaying {
                title: "Şımarık".into(),
                ..np()
            },
            1_000_000,
        );
        assert!(
            !worth_sending(a.as_ref(), tick.as_ref()),
            "a normal tick is not a change"
        );
        assert!(worth_sending(a.as_ref(), seek.as_ref()));
        assert!(worth_sending(a.as_ref(), other.as_ref()));
        assert!(worth_sending(a.as_ref(), None));
        assert!(!worth_sending(None, None));
    }
}
