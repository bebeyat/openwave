//! UI state of the player surfaces (queue drawer, fullscreen, shortcut popover) and the mapping of
//! keyboard shortcuts to playback actions.

use dioxus::prelude::*;

use crate::features::shell::shortcuts::Action;
use crate::features::tracks::state::{library, toggle_like};
use crate::player::controller as ctl;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerTab {
    Queue,
    Lyrics,
}

/// The right-hand drawer: `None` is closed.
pub static DRAWER: GlobalSignal<Option<DrawerTab>> = Signal::global(|| None);
/// Tab shown while the drawer slides out (when `DRAWER` is already `None`).
pub static LAST_TAB: GlobalSignal<DrawerTab> = Signal::global(|| DrawerTab::Queue);
pub static FULLSCREEN: GlobalSignal<bool> = Signal::global(|| false);
/// Lyrics pane inside the fullscreen player.
pub static FULLSCREEN_LYRICS: GlobalSignal<bool> = Signal::global(|| true);
/// Frequency bars in the fullscreen player.
pub static FULLSCREEN_VIZ: GlobalSignal<bool> = Signal::global(|| false);
/// Window is at least 1280px wide: the queue/lyrics panel docks into the shell's right column.
pub static WIDE: GlobalSignal<bool> = Signal::global(|| false);
pub static SHORTCUTS_OPEN: GlobalSignal<bool> = Signal::global(|| false);
pub static SLEEP_OPEN: GlobalSignal<bool> = Signal::global(|| false);

pub const SEEK_STEP_SECS: f64 = 5.0;
pub const VOLUME_STEP: f64 = 0.05;

/// DOM ids used to return focus after a surface closes.
pub const QUEUE_BUTTON_ID: &str = "ow-queue-toggle";
pub const LYRICS_BUTTON_ID: &str = "ow-lyrics-toggle";

/// Open `tab`, or close the drawer when it already shows that tab.
pub fn toggle_drawer(tab: DrawerTab) {
    let next = if *DRAWER.peek() == Some(tab) {
        None
    } else {
        Some(tab)
    };
    if let Some(tab) = next {
        *LAST_TAB.write() = tab;
    }
    *DRAWER.write() = next;
}

/// The panel is docked (not a slide-over): wide window and not in the fullscreen player.
pub fn docked() -> bool {
    *WIDE.read() && !*FULLSCREEN.read()
}

/// The shell shows the docked player panel in its right column.
pub fn docked_panel_open() -> bool {
    docked() && DRAWER.read().is_some()
}

pub fn toggle_fullscreen() {
    let on = !*FULLSCREEN.peek();
    *FULLSCREEN.write() = on;
}

/// Close the topmost open surface. Returns whether something was closed (Esc handling).
pub fn close_top_overlay() -> bool {
    if *SHORTCUTS_OPEN.peek() {
        *SHORTCUTS_OPEN.write() = false;
    } else if DRAWER.peek().is_some() {
        *DRAWER.write() = None;
    } else if *FULLSCREEN.peek() {
        *FULLSCREEN.write() = false;
    } else {
        return false;
    }
    true
}

pub fn toggle_like_current() {
    if let Some(track) = ctl::current_track() {
        toggle_like(&track);
    }
}

pub fn is_current_liked() -> bool {
    ctl::QUEUE
        .read()
        .current()
        .is_some_and(|e| library().is_liked(&e.track.id))
}

/// Apply a global shortcut. Escape and search focus belong to the shell.
pub fn handle_shortcut(action: Action) {
    let has_track = ctl::current_track().is_some();
    match action {
        Action::ToggleQueue => toggle_drawer(DrawerTab::Queue),
        Action::ToggleLyrics => {
            if *FULLSCREEN.peek() {
                let on = !*FULLSCREEN_LYRICS.peek();
                *FULLSCREEN_LYRICS.write() = on;
            } else {
                toggle_drawer(DrawerTab::Lyrics);
            }
        }
        Action::Fullscreen if has_track => toggle_fullscreen(),
        Action::PlayPause => ctl::toggle_pause(),
        Action::SeekBack => ctl::seek_by(-SEEK_STEP_SECS),
        Action::SeekForward => ctl::seek_by(SEEK_STEP_SECS),
        Action::Previous => ctl::previous(),
        Action::Next => ctl::next(),
        Action::VolumeUp => ctl::volume_by(VOLUME_STEP),
        Action::VolumeDown => ctl::volume_by(-VOLUME_STEP),
        Action::Mute => ctl::toggle_mute(),
        Action::Like => toggle_like_current(),
        Action::Fullscreen | Action::FocusSearch | Action::Escape => {}
    }
}
