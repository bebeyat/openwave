//! Live numbers for the running download: bytes so far, speed and time left (issue #40).

use std::time::{Duration, Instant};

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Live {
    pub got: u64,
    /// 0 while the size is not known yet.
    pub total: u64,
    pub bytes_per_sec: f64,
}

pub static LIVE: GlobalSignal<Live> = Signal::global(Live::default);

const TICK: Duration = Duration::from_millis(500);

/// Sample the backend's byte counter twice a second while `running()` holds.
pub(super) fn start(running: fn() -> bool) {
    spawn_forever(async move {
        let mut last = (0_u64, Instant::now());
        while running() {
            tokio::time::sleep(TICK).await;
            let (got, total) = crate::backend::media::download_progress();
            let now = Instant::now();
            let speed = next_speed(LIVE.peek().bytes_per_sec, last.0, got, now - last.1);
            last = (got, now);
            *LIVE.write() = Live {
                got,
                total,
                bytes_per_sec: speed,
            };
        }
        *LIVE.write() = Live::default();
    });
}

/// Smoothed speed; when a new song starts (fewer bytes than before) the old estimate stays.
fn next_speed(prev: f64, before: u64, now: u64, dt: Duration) -> f64 {
    let secs = dt.as_secs_f64();
    if now < before || secs <= 0.0 {
        return prev;
    }
    let sample = (now - before) as f64 / secs;
    if prev == 0.0 {
        sample
    } else {
        prev * 0.7 + sample * 0.3
    }
}

/// How much of the running song is downloaded, once its size is known.
pub fn percent(l: Live) -> Option<u64> {
    (l.got * 100).checked_div(l.total).map(|p| p.min(100))
}

/// "1.2 MB/s" / "340 KB/s".
pub fn speed_text(bytes_per_sec: f64) -> String {
    let mb = bytes_per_sec / (1024.0 * 1024.0);
    if mb >= 1.0 {
        format!("{mb:.1} MB/s")
    } else {
        format!("{} KB/s", (bytes_per_sec / 1024.0).round() as u64)
    }
}

/// Seconds left for the whole run: the rest of this song plus `waiting` songs of the same size.
pub fn eta_secs(l: Live, waiting: usize) -> Option<f64> {
    if l.bytes_per_sec < 1.0 || l.total == 0 {
        return None;
    }
    let left = l.total.saturating_sub(l.got) + l.total * waiting as u64;
    Some(left as f64 / l.bytes_per_sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_is_smoothed_and_survives_a_new_song() {
        let s = Duration::from_secs(1);
        assert_eq!(next_speed(0.0, 0, 1000, s), 1000.0);
        assert_eq!(next_speed(1000.0, 1000, 3000, s), 1300.0);
        assert_eq!(next_speed(1300.0, 5000, 10, s), 1300.0, "counter reset");
    }

    #[test]
    fn percent_and_eta_need_a_known_size() {
        let unknown = Live {
            got: 5,
            total: 0,
            bytes_per_sec: 10.0,
        };
        assert_eq!(percent(unknown), None);
        assert_eq!(eta_secs(unknown, 3), None);
        let half = Live {
            got: 50,
            total: 100,
            bytes_per_sec: 10.0,
        };
        assert_eq!(percent(half), Some(50));
        assert_eq!(eta_secs(half, 2), Some(25.0));
        assert_eq!(speed_text(1.5 * 1024.0 * 1024.0), "1.5 MB/s");
        assert_eq!(speed_text(340.0 * 1024.0), "340 KB/s");
    }
}
