//! The player feature: bar, queue/lyrics drawer, fullscreen player and keyboard wiring.
//! `PlayerRoot` is mounted once by the shell (inside `PlayerBarSlot`).

mod bar;
mod controls;
mod drawer;
mod effects_eq;
mod effects_parts;
pub mod effects_popover;
mod fullscreen;
mod lyric_logic;
mod lyrics;
mod lyrics_state;
mod lyrics_tools;
mod mini;
mod queue_list;
mod save_dialog;
mod shortcuts_popover;
mod sleep_popover;
pub mod state;

pub use drawer::DockedPanel;
pub use state::docked_panel_open;

use dioxus::desktop::use_window;
use dioxus::prelude::*;

use crate::features::shell::shortcuts::use_shortcuts;
use crate::player::{controller, engine, media_keys, session};

/// Starts the audio bridge, restores the last session, connects media keys and renders every
/// player surface.
#[component]
pub fn PlayerRoot() -> Element {
    let window = use_window();
    let w_mini = window.clone();
    use_hook(session::restore);
    use_future(|| async {
        engine::run_events(controller::handle_event).await;
    });
    use_future(move || {
        // Media keys need the native window handle on Windows; elsewhere `run` simply returns.
        #[cfg(windows)]
        let hwnd = {
            use dioxus::desktop::tao::platform::windows::WindowExtWindows;
            window.hwnd()
        };
        #[cfg(not(windows))]
        let hwnd = {
            let _ = &window;
            0
        };
        async move { media_keys::run(hwnd).await }
    });
    use_shortcuts(move |action| match action {
        crate::features::shell::shortcuts::Action::Mini => {
            crate::features::shell::window::toggle_mini(&w_mini)
        }
        other => state::handle_shortcut(other),
    });
    use_future(|| async {
        let mut ev = document::eval(
            "var q=window.matchMedia('(min-width: 1280px)');dioxus.send(q.matches);q.addEventListener('change',function(e){dioxus.send(e.matches)});",
        );
        while let Ok(wide) = ev.recv::<bool>().await {
            if *state::WIDE.peek() != wide {
                *state::WIDE.write() = wide;
            }
        }
    });
    #[cfg(debug_assertions)]
    debug_autoplay();
    rsx! {
        bar::PlayerBar {}
        drawer::Drawer {}
        effects_popover::FxPopover {}
        fullscreen::FullscreenPlayer {}
        mini::MiniPlayer {}
    }
}

/// Debug builds only: `OPENWAVE_PLAY="daft punk one more time"` searches and plays the first
/// hit; `OPENWAVE_PLAY_ALL=1` queues every hit, `OPENWAVE_PLAY_BAD=1` puts an unplayable song first. Makes UI tests independent of the search page.
#[cfg(debug_assertions)]
fn debug_autoplay() {
    use_future(|| async {
        let Ok(term) = std::env::var("OPENWAVE_PLAY") else {
            return;
        };
        let found =
            crate::backend::api::search(crate::backend::api::SearchKind::Track, &term).await;
        match found {
            Ok(r) if !r.tracks.is_empty() => {
                let all = std::env::var("OPENWAVE_PLAY_ALL").is_ok();
                let mut tracks = if all {
                    r.tracks
                } else {
                    r.tracks[..1].to_vec()
                };
                if std::env::var("OPENWAVE_PLAY_BAD").is_ok() {
                    // An unplayable song first, to exercise retry -> toast -> skip.
                    let bad = crate::model::Track {
                        id: "aaaaaaaaaaa".into(),
                        title: "Broken".into(),
                        ..Default::default()
                    };
                    tracks.insert(0, bad);
                }
                controller::play_now(&tracks, 0);
            }
            other => eprintln!("OPENWAVE_PLAY: nothing to play: {other:?}"),
        }
    });
}
