//! Small pure rules of the player, kept separate so they can be unit tested.

/// "Previous" restarts the song instead of going back when it has played longer than this.
pub const RESTART_AFTER_SECS: f64 = 3.0;
/// Most songs in a row that may fail before playback stops instead of skipping again.
pub const MAX_FAIL_STREAK: u32 = 3;
/// How often the playing position is written to disk (seconds).
pub const PERSIST_EVERY_MS: u64 = 10_000;
/// Quiet time before a queue change is written to disk.
pub const PERSIST_DEBOUNCE_MS: u64 = 1_500;
/// Similar songs requested when the queue ends.
pub const RADIO_BATCH: usize = 25;

/// True when "previous" should seek to 0 rather than go to the previous song.
pub fn previous_restarts(position_secs: f64) -> bool {
    position_secs > RESTART_AFTER_SECS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Ask for a fresh stream address and try the same song once more.
    Retry,
    /// Tell the user and move on to the next song.
    Skip,
    /// Too many failures in a row: stop and let the user decide.
    GiveUp,
}

/// What to do after a stream failure. `retried` is whether this song already had its retry;
/// `streak` counts songs that failed in a row *before* this one.
pub fn failure_step(retried: bool, streak: u32) -> Failure {
    if !retried {
        Failure::Retry
    } else if streak + 1 >= MAX_FAIL_STREAK {
        Failure::GiveUp
    } else {
        Failure::Skip
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    /// The source is loaded: seek to 0 and play.
    Seek,
    /// The source is gone (failed, stopped, never loaded): load the song again.
    Reload,
    Nothing,
}

/// How to play the current song again from the start.
pub fn restart_plan(source_loaded: bool, has_track: bool) -> Restart {
    match (has_track, source_loaded) {
        (false, _) => Restart::Nothing,
        (true, true) => Restart::Seek,
        (true, false) => Restart::Reload,
    }
}

/// Whether the position should be written to disk again.
pub fn persist_due(now_ms: u64, last_ms: u64) -> bool {
    now_ms.saturating_sub(last_ms) >= PERSIST_EVERY_MS
}

/// Position (ms) to store: whole seconds are enough, never beyond the track length when known.
pub fn snapshot_position_ms(position_secs: f64, duration_ms: u64) -> u64 {
    let ms = if position_secs.is_finite() && position_secs > 0.0 {
        (position_secs * 1000.0) as u64
    } else {
        0
    };
    if duration_ms > 0 {
        ms.min(duration_ms)
    } else {
        ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_restart_threshold() {
        assert!(!previous_restarts(3.0));
        assert!(previous_restarts(3.1));
        assert!(!previous_restarts(0.0));
    }

    #[test]
    fn restart_reloads_a_dead_source_instead_of_seeking_it() {
        assert_eq!(restart_plan(true, true), Restart::Seek);
        assert_eq!(restart_plan(false, true), Restart::Reload);
        assert_eq!(restart_plan(true, false), Restart::Nothing);
        assert_eq!(restart_plan(false, false), Restart::Nothing);
    }

    #[test]
    fn failure_flow_retries_once_then_skips_then_gives_up() {
        assert_eq!(failure_step(false, 0), Failure::Retry);
        assert_eq!(failure_step(true, 0), Failure::Skip);
        assert_eq!(failure_step(true, 1), Failure::Skip);
        assert_eq!(failure_step(true, 2), Failure::GiveUp);
        assert_eq!(failure_step(false, 5), Failure::Retry);
    }

    #[test]
    fn persist_timing() {
        assert!(!persist_due(5_000, 0));
        assert!(persist_due(10_000, 0));
        assert!(!persist_due(0, 10_000));
    }

    #[test]
    fn snapshot_position_is_bounded() {
        assert_eq!(snapshot_position_ms(12.34, 200_000), 12_340);
        assert_eq!(snapshot_position_ms(999.0, 200_000), 200_000);
        assert_eq!(snapshot_position_ms(-1.0, 200_000), 0);
        assert_eq!(snapshot_position_ms(f64::NAN, 0), 0);
        assert_eq!(snapshot_position_ms(5.0, 0), 5_000);
    }
}
