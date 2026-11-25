//! The two halves of a session: the host turning its playback into messages ([`Publish`]) and a
//! guest catching up with them ([`Follow`]).

use std::time::{Duration, Instant};

use dioxus::prelude::*;

use crate::backend::listen::{Msg, MAX_QUEUE};
use crate::model::{Id, Track};
use crate::player::controller as ctl;

/// The host repeats its state this often, so late guests start and drift gets corrected.
const REPEAT: Duration = Duration::from_secs(5);
/// A position change bigger than playback explains is a seek.
const JUMP_SECS: f64 = 2.5;
/// A guest further off than this seeks.
const DRIFT_SECS: f64 = 2.0;
/// Clocks of two computers are not exact: never move a guest ahead by more than this.
const MAX_LAG_SECS: f64 = 10.0;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn ids(tracks: &[Track]) -> Vec<Id> {
    tracks.iter().map(|t| t.id.clone()).collect()
}

#[derive(Default)]
pub(super) struct Publish {
    seq: u64,
    uid: Option<u64>,
    play: bool,
    pos: f64,
    ids: Vec<Id>,
    read_at: Option<Instant>,
    sent_at: Option<Instant>,
}

impl Publish {
    /// The state to send now, if any: at once after a change, else every [`REPEAT`].
    pub(super) fn tick(&mut self) -> Option<Msg> {
        let queue = ctl::QUEUE.peek();
        let uid = queue.current_uid();
        // A file from this computer cannot play elsewhere: guests get nothing to play.
        let q: Vec<Track> = match queue.current() {
            Some(cur) if !cur.track.is_local() => std::iter::once(cur)
                .chain(queue.upcoming().iter().filter(|e| !e.track.is_local()))
                .take(MAX_QUEUE)
                .map(|e| e.track.clone())
                .collect(),
            _ => Vec::new(),
        };
        drop(queue);
        let play = ctl::STATUS.peek().is_active();
        let pos = *ctl::POSITION.peek();
        let now = Instant::now();
        let since = self.read_at.map_or(0.0, |t| (now - t).as_secs_f64());
        let expected = self.pos + if self.play { since } else { 0.0 };
        let q_ids = ids(&q);
        let changed = uid != self.uid
            || play != self.play
            || (pos - expected).abs() > JUMP_SECS
            || q_ids != self.ids;
        if changed {
            self.seq += 1;
        }
        (self.uid, self.play, self.pos, self.ids, self.read_at) =
            (uid, play, pos, q_ids, Some(now));
        let due = self.sent_at.is_none_or(|t| now - t >= REPEAT);
        (changed || due).then(|| {
            self.sent_at = Some(now);
            Msg::State {
                seq: self.seq,
                at: now_ms(),
                pos,
                play,
                q,
            }
        })
    }
}

#[derive(Default)]
pub(super) struct Follow {
    seq: Option<u64>,
    at: u64,
}

impl Follow {
    /// Catch up with a state from the host. The song always follows; play/pause follows only
    /// when the host changed something, so a guest can pause for a moment.
    pub(super) fn apply(&mut self, seq: u64, at: u64, pos: f64, play: bool, q: &[Track]) {
        // Relays may deliver an older message late.
        if at < self.at {
            return;
        }
        self.at = at;
        let fresh = self.seq != Some(seq);
        self.seq = Some(seq);
        let lag = if play {
            (now_ms().saturating_sub(at) as f64 / 1000.0).min(MAX_LAG_SECS)
        } else {
            0.0
        };
        let target = pos + lag;
        let Some(first) = q.first() else {
            ctl::pause();
            return;
        };
        if ctl::current_track().map(|t| t.id) != Some(first.id.clone()) {
            ctl::play_now_at(q, target, play);
            return;
        }
        let upcoming: Vec<Id> = ctl::QUEUE
            .peek()
            .upcoming()
            .iter()
            .map(|e| e.track.id.clone())
            .collect();
        if upcoming != ids(&q[1..]) {
            ctl::replace_upcoming(&q[1..]);
        }
        if fresh {
            if play {
                ctl::resume();
            } else {
                ctl::pause();
            }
        }
        let following = fresh || (play && ctl::STATUS.peek().is_active());
        if following && (*ctl::POSITION.peek() - target).abs() > DRIFT_SECS {
            ctl::seek_to(target);
        }
    }
}
