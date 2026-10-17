//! Reacting to messages from the audio bridge.

use super::controller::{
    advance, current_track, on_failure, pause, set_status, with_core, Status, DURATION, MUTED,
    POSITION, STATUS, VOLUME,
};
use super::engine::{AudioEvent, AudioState};
use super::session;
use crate::backend::error::MediaError;
use crate::backend::Coded;
use crate::store;
use dioxus::prelude::*;

/// Apply one message from the audio bridge.
pub fn handle_event(event: AudioEvent) {
    if let Some(g) = event.generation() {
        if g != with_core(|c| c.generation) {
            return;
        }
    }
    match event {
        AudioEvent::Time { pos, dur, .. } => on_time(pos, dur),
        AudioEvent::State { s, .. } => on_state(s),
        AudioEvent::Error { g, .. } => {
            if let Some(track) = current_track() {
                let at = *POSITION.peek();
                on_failure(g, &track, at, MediaError::Unavailable.user_message());
            }
        }
        AudioEvent::Vol { v, muted } => {
            *VOLUME.write() = v;
            *MUTED.write() = muted;
        }
        AudioEvent::FxFailed => {
            crate::ui::toast_error(crate::i18n::t("fx.error.unavailable"));
        }
        AudioEvent::DeviceLost => {
            if store::settings().get().pause_on_device_change {
                pause();
            }
        }
    }
}

fn on_time(pos: f64, dur: Option<f64>) {
    *POSITION.write() = pos;
    if let Some(d) = dur {
        if (*DURATION.peek() - d).abs() > 0.5 {
            *DURATION.write() = d;
        }
    }
    session::position_tick();
    super::effects::refresh_overlay(pos);
}

fn on_state(state: AudioState) {
    let active = STATUS.peek().is_active();
    match state {
        AudioState::Playing => {
            set_status(Status::Playing);
            session::record_play_once();
            with_core(|c| {
                c.fail_streak = 0;
                c.retried = false;
            });
        }
        AudioState::Buffering if active => set_status(Status::Buffering),
        AudioState::Paused if active => {
            set_status(Status::Paused);
            session::persist_now();
        }
        AudioState::Ended => advance(true),
        _ => {}
    }
}
