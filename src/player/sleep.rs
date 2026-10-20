//! Sleep timer: pause after some minutes (with a short fade) or when the current song ends.

use std::cell::Cell;
use std::time::{Duration, Instant};

use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use serde_json::json;

use super::engine;

/// Length of the volume fade before the pause.
const FADE: Duration = Duration::from_secs(4);
/// The remaining-minutes readout refreshes at least this often.
const TICK: Duration = Duration::from_secs(15);
pub const MINUTE_CHOICES: [u64; 5] = [15, 30, 45, 60, 90];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sleep {
    At(Instant),
    EndOfTrack,
}

pub static SLEEP: GlobalSignal<Option<Sleep>> = Signal::global(|| None);
/// Whole minutes left (rounded up) for the readout; `None` when no countdown runs.
pub static MINUTES_LEFT: GlobalSignal<Option<u64>> = Signal::global(|| None);

thread_local! {
    /// Bumped on every change so an older countdown task stops.
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

/// Minutes left at `now`, rounded up (a countdown that has 10 s left still says 1).
pub fn minutes_left(at: Instant, now: Instant) -> u64 {
    at.saturating_duration_since(now).as_secs().div_ceil(60)
}

fn bump() -> u64 {
    let g = GENERATION.get() + 1;
    GENERATION.set(g);
    g
}

pub fn cancel() {
    bump();
    *SLEEP.write() = None;
    *MINUTES_LEFT.write() = None;
}

pub fn end_of_track() {
    bump();
    *SLEEP.write() = Some(Sleep::EndOfTrack);
    *MINUTES_LEFT.write() = None;
}

pub fn after_minutes(minutes: u64) {
    let gen = bump();
    let at = Instant::now() + Duration::from_secs(minutes * 60);
    *SLEEP.write() = Some(Sleep::At(at));
    *MINUTES_LEFT.write() = Some(minutes);
    spawn_forever(async move {
        loop {
            let now = Instant::now();
            if GENERATION.get() != gen {
                return;
            }
            let fade_at = at.checked_sub(FADE).unwrap_or(at);
            if now >= fade_at {
                break;
            }
            *MINUTES_LEFT.write() = Some(minutes_left(at, now));
            tokio::time::sleep(TICK.min(fade_at - now)).await;
        }
        engine::send(json!({ "c": "fade_pause", "secs": FADE.as_secs_f64() }));
        tokio::time::sleep(FADE).await;
        if GENERATION.get() == gen {
            *SLEEP.write() = None;
            *MINUTES_LEFT.write() = None;
        }
    });
}

/// Called when a song ends on its own: `true` means stop here instead of moving on.
pub fn stop_at_track_end() -> bool {
    if *SLEEP.peek() != Some(Sleep::EndOfTrack) {
        return false;
    }
    cancel();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_round_up() {
        let now = Instant::now();
        assert_eq!(minutes_left(now + Duration::from_secs(10), now), 1);
        assert_eq!(minutes_left(now + Duration::from_secs(60), now), 1);
        assert_eq!(minutes_left(now + Duration::from_secs(61), now), 2);
        assert_eq!(minutes_left(now, now + Duration::from_secs(5)), 0);
    }
}
