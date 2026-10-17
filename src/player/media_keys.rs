//! Windows media keys and the system media overlay (SMTC) through the `souvlaki` crate
//! (MIT licensed). The feature is optional: if the OS refuses to create the controls the app
//! simply plays without them (the reason is printed once to stderr).
//!
//! The OS calls back on its own thread, so events are forwarded through a channel and applied
//! on the UI thread by [`run`].

use std::cell::RefCell;
use std::time::Duration;

use dioxus::prelude::ReadableExt;
use tokio::sync::mpsc::UnboundedReceiver;

use super::controller::{self, Status, POSITION};
use crate::model::Track;

/// What the OS asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekBy(f64),
    SeekTo(f64),
}

/// Seconds skipped by the overlay's fast-forward / rewind buttons.
const SKIP_SECS: f64 = 10.0;

#[cfg(windows)]
mod platform {
    use super::*;
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig, SeekDirection,
    };
    use std::ffi::c_void;

    thread_local! {
        static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
    }

    pub fn map(event: MediaControlEvent) -> Option<Key> {
        Some(match event {
            MediaControlEvent::Play => Key::Play,
            MediaControlEvent::Pause => Key::Pause,
            MediaControlEvent::Toggle => Key::Toggle,
            MediaControlEvent::Next => Key::Next,
            MediaControlEvent::Previous => Key::Previous,
            MediaControlEvent::Stop => Key::Stop,
            MediaControlEvent::Seek(SeekDirection::Forward) => Key::SeekBy(SKIP_SECS),
            MediaControlEvent::Seek(SeekDirection::Backward) => Key::SeekBy(-SKIP_SECS),
            MediaControlEvent::SeekBy(SeekDirection::Forward, d) => Key::SeekBy(d.as_secs_f64()),
            MediaControlEvent::SeekBy(SeekDirection::Backward, d) => Key::SeekBy(-d.as_secs_f64()),
            MediaControlEvent::SetPosition(MediaPosition(d)) => Key::SeekTo(d.as_secs_f64()),
            _ => return None,
        })
    }

    pub fn init(hwnd: isize) -> Result<UnboundedReceiver<Key>, String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let config = PlatformConfig {
            dbus_name: "openwave",
            display_name: "OpenWave",
            hwnd: Some(hwnd as *mut c_void),
        };
        let mut controls = MediaControls::new(config).map_err(|e| e.to_string())?;
        controls
            .attach(move |event| {
                if let Some(key) = map(event) {
                    let _ = tx.send(key);
                }
            })
            .map_err(|e| e.to_string())?;
        CONTROLS.with(|c| *c.borrow_mut() = Some(controls));
        Ok(rx)
    }

    pub fn set_track(track: Option<&Track>) {
        CONTROLS.with(|c| {
            let mut guard = c.borrow_mut();
            let Some(controls) = guard.as_mut() else {
                return;
            };
            let artist = track.map(Track::artists_label).unwrap_or_default();
            let meta = match track {
                Some(t) => MediaMetadata {
                    title: Some(t.title.as_str()),
                    artist: Some(artist.as_str()),
                    album: t.album.as_ref().map(|a| a.title.as_str()),
                    cover_url: t.cover.as_ref().map(|c| c.url.as_str()),
                    duration: (t.duration_ms > 0).then(|| Duration::from_millis(t.duration_ms)),
                },
                None => MediaMetadata::default(),
            };
            if let Err(e) = controls.set_metadata(meta) {
                eprintln!("media overlay: {e}");
            }
        });
    }

    pub fn set_playback(status: Status, position_secs: f64) {
        CONTROLS.with(|c| {
            let mut guard = c.borrow_mut();
            let Some(controls) = guard.as_mut() else {
                return;
            };
            let progress = Some(MediaPosition(Duration::from_secs_f64(
                position_secs.max(0.0),
            )));
            let playback = match status {
                Status::Playing | Status::Buffering => MediaPlayback::Playing { progress },
                Status::Paused | Status::Loading | Status::FindingSimilar => {
                    MediaPlayback::Paused { progress }
                }
                Status::Idle => MediaPlayback::Stopped,
            };
            let _ = controls.set_playback(playback);
        });
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub fn init(_hwnd: isize) -> Result<UnboundedReceiver<Key>, String> {
        Err("media keys are only supported on Windows".into())
    }
    pub fn set_track(_track: Option<&Track>) {}
    pub fn set_playback(_status: Status, _position_secs: f64) {}
}

thread_local! {
    static ACTIVE: RefCell<bool> = const { RefCell::new(false) };
}

/// Create the OS controls for the window and apply their events until the app closes.
/// Failure is not fatal: the player works without media keys.
pub async fn run(hwnd: isize) {
    let mut rx = match platform::init(hwnd) {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("media keys unavailable: {e}");
            return;
        }
    };
    ACTIVE.with(|a| *a.borrow_mut() = true);
    update_track(controller::current_track().as_ref());
    update_playback(*controller::STATUS.peek(), *POSITION.peek());
    while let Some(key) = rx.recv().await {
        apply(key);
    }
}

fn apply(key: Key) {
    match key {
        Key::Play => controller::resume(),
        Key::Pause | Key::Stop => controller::pause(),
        Key::Toggle => controller::toggle_pause(),
        Key::Next => controller::next(),
        Key::Previous => controller::previous(),
        Key::SeekBy(d) => controller::seek_by(d),
        Key::SeekTo(p) => controller::seek_to(p),
    }
}

fn active() -> bool {
    ACTIVE.with(|a| *a.borrow())
}

/// Tell the overlay which song is current (`None` clears it).
pub fn update_track(track: Option<&Track>) {
    if active() {
        platform::set_track(track);
    }
}

/// Tell the overlay whether audio is playing.
pub fn update_playback(status: Status, position_secs: f64) {
    if active() {
        platform::set_playback(status, position_secs);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use souvlaki::{MediaControlEvent, MediaPosition, SeekDirection};

    #[test]
    fn maps_overlay_events() {
        assert_eq!(platform::map(MediaControlEvent::Next), Some(Key::Next));
        assert_eq!(platform::map(MediaControlEvent::Toggle), Some(Key::Toggle));
        assert_eq!(
            platform::map(MediaControlEvent::Seek(SeekDirection::Backward)),
            Some(Key::SeekBy(-SKIP_SECS))
        );
        assert_eq!(
            platform::map(MediaControlEvent::SetPosition(MediaPosition(
                Duration::from_secs(42)
            ))),
            Some(Key::SeekTo(42.0))
        );
        assert_eq!(platform::map(MediaControlEvent::Raise), None);
    }
}
