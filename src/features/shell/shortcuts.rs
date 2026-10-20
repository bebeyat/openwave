//! Global keyboard shortcuts.
//!
//! A document-level JS listener (`ui::js::SHORTCUT_JS`) decides *whether* a key press is a shortcut
//! (it ignores typing in inputs, ignores Space/Enter on buttons and other activatable controls,
//! leaves arrows to sliders/tabs/menus, and stays silent while a dialog/menu is open) and then
//! reports a name. It never calls `blur()`, so focused buttons keep focus.
//!
//! The shell publishes each action on `SHORTCUT`. Feature code subscribes with
//! `use_shortcuts(|action| ...)`. Focus-search (`/`), the command palette (Ctrl+K) and Esc-to-go-back are handled by the shell.

use dioxus::prelude::*;

use crate::ui::js::SHORTCUT_JS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayPause,
    SeekBack,
    SeekForward,
    Previous,
    Next,
    VolumeUp,
    VolumeDown,
    Mute,
    ToggleQueue,
    ToggleLyrics,
    Fullscreen,
    Like,
    FocusSearch,
    Escape,
    Visualizer,
    Mini,
    Palette,
}

impl Action {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "play_pause" => Action::PlayPause,
            "seek_back" => Action::SeekBack,
            "seek_forward" => Action::SeekForward,
            "previous" => Action::Previous,
            "next" => Action::Next,
            "volume_up" => Action::VolumeUp,
            "volume_down" => Action::VolumeDown,
            "mute" => Action::Mute,
            "queue" => Action::ToggleQueue,
            "lyrics" => Action::ToggleLyrics,
            "fullscreen" => Action::Fullscreen,
            "like" => Action::Like,
            "focus_search" => Action::FocusSearch,
            "escape" => Action::Escape,
            "visualizer" => Action::Visualizer,
            "mini" => Action::Mini,
            "palette" => Action::Palette,
            _ => return None,
        })
    }
}

/// Recent actions with sequence numbers, so repeated presses are distinct events and presses that
/// land before the effects run are not lost. Only the last [`SHORTCUT_LOG`] are kept.
pub static SHORTCUT: GlobalSignal<Vec<(u64, Action)>> = Signal::global(Vec::new);
const SHORTCUT_LOG: usize = 32;

/// Starts the JS listener and forwards actions to `SHORTCUT`. Call once (the shell does).
pub fn use_shortcut_bridge() {
    use_future(|| async move {
        let mut eval = document::eval(SHORTCUT_JS);
        let mut seq = 0_u64;
        while let Ok(name) = eval.recv::<String>().await {
            if let Some(action) = Action::parse(&name) {
                seq += 1;
                let mut log = SHORTCUT.write();
                if log.len() == SHORTCUT_LOG {
                    log.remove(0);
                }
                log.push((seq, action));
            }
        }
    });
}

/// Runs `handler` for every shortcut action while the calling component is mounted.
pub fn use_shortcuts(mut handler: impl FnMut(Action) + 'static) {
    let mut last_seen = use_signal(|| SHORTCUT.peek().last().map_or(0, |&(n, _)| n));
    use_effect(move || {
        let seen = *last_seen.peek();
        let fresh: Vec<_> = SHORTCUT
            .read()
            .iter()
            .filter(|&&(n, _)| n > seen)
            .copied()
            .collect();
        if let Some(&(n, _)) = fresh.last() {
            last_seen.set(n);
        }
        for (_, action) in fresh {
            handler(action);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_names_only() {
        assert_eq!(Action::parse("play_pause"), Some(Action::PlayPause));
        assert_eq!(Action::parse("escape"), Some(Action::Escape));
        assert_eq!(Action::parse("nope"), None);
    }
}
