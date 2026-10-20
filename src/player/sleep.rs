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
pub const MINUTES_MIN: u64 = 1;
pub const MINUTES_MAX: u64 = 720;
pub const MINUTES_STEP: u64 = 5;
pub const MINUTES_DEFAULT: u64 = 30;

/// `minutes` moved one step up or down, landing on whole steps (7 -> 10 up, 7 -> 5 down).
pub fn step_minutes(minutes: u64, up: bool) -> u64 {
    let next = if up {
        (minutes / MINUTES_STEP + 1) * MINUTES_STEP
    } else {
        minutes.saturating_sub(1) / MINUTES_STEP * MINUTES_STEP
    };
    next.clamp(MINUTES_MIN, MINUTES_MAX)
}

/// "29:58", or "1:05:09" from an hour up.
pub fn countdown_text(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Typed minutes, clamped to the allowed range; `None` when it is not a number.
pub fn parse_minutes(text: &str) -> Option<u64> {
    let n: u64 = text.trim().parse().ok()?;
    Some(n.clamp(MINUTES_MIN, MINUTES_MAX))
}

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

/// A new choice replaces the old one: stop the old countdown and any fade it started.
fn bump() -> u64 {
    let g = GENERATION.get() + 1;
    GENERATION.set(g);
    engine::send(json!({ "c": "fade_cancel" }));
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

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("sleep_tests.rs");
}
