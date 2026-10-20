//! Command palette (Ctrl+K): type to filter app commands, Enter runs one; any text can also be
//! searched for directly.

use dioxus::desktop::{use_window, DesktopContext};
use dioxus::prelude::*;

use crate::features::player::effects_popover::FX_OPEN;
use crate::features::player::state::{toggle_fullscreen, toggle_like_current, SLEEP_OPEN};
use crate::features::shell::slots::OPEN_IMPORT;
use crate::i18n::t;
use crate::player::controller as ctl;
use crate::routes::Route;
use crate::ui::Icon;

pub static PALETTE_OPEN: GlobalSignal<bool> = Signal::global(|| false);

const INPUT_ID: &str = "ow-palette-input";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Home,
    Library,
    Liked,
    History,
    Stats,
    Settings,
    PlayPause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    Like,
    Fullscreen,
    Mini,
    Sleep,
    Effects,
    Import,
}

/// (command, i18n key of its name, icon), in display order.
const COMMANDS: [(Command, &str, &str); 17] = [
    (Command::PlayPause, "tray.play_pause", "play"),
    (Command::Next, "tray.next", "skip-forward"),
    (Command::Previous, "tray.previous", "skip-back"),
    (Command::Shuffle, "player.shuffle", "shuffle"),
    (Command::Repeat, "player.repeat", "repeat"),
    (Command::Like, "player.like", "heart"),
    (Command::Fullscreen, "player.fullscreen", "maximize-2"),
    (Command::Mini, "player.mini", "picture-in-picture-2"),
    (Command::Sleep, "sleep.title", "moon"),
    (Command::Effects, "fx.title", "sliders-horizontal"),
    (Command::Home, "nav.home", "house"),
    (Command::Library, "nav.library", "library"),
    (Command::Liked, "library.tab.liked", "heart"),
    (Command::History, "library.tab.history", "history"),
    (Command::Stats, "stats.title", "chart-column"),
    (Command::Settings, "nav.settings", "settings"),
    (Command::Import, "palette.import", "download"),
];

/// One row of the list: a command, or "search for <text>".
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Run(Command, String, &'static str),
    Search(String),
}

/// Commands whose name contains every word of `query` (case-insensitive), then the search row.
fn items(query: &str) -> Vec<Item> {
    let q = query.trim().to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    let commands = COMMANDS.iter().filter_map(|(c, key, icon)| {
        let name = t(key);
        let low = name.to_lowercase();
        words
            .iter()
            .all(|w| low.contains(w))
            .then_some(Item::Run(*c, name, icon))
    });
    let search = (!q.is_empty()).then(|| Item::Search(query.trim().to_owned()));
    commands.chain(search).collect()
}

fn library(tab: &str) -> Route {
    Route::Library { tab: tab.into() }
}

fn run(item: &Item, window: &DesktopContext) {
    close();
    let nav = navigator();
    let has_track = ctl::current_track().is_some();
    match item {
        Item::Search(q) => {
            nav.push(Route::Search { q: q.clone() });
        }
        Item::Run(c, ..) => match c {
            Command::Home => {
                nav.push(Route::Home {});
            }
            Command::Library => {
                nav.push(library("playlists"));
            }
            Command::Liked => {
                nav.push(library("liked"));
            }
            Command::History => {
                nav.push(library("history"));
            }
            Command::Stats => {
                nav.push(Route::Stats {});
            }
            Command::Settings => {
                nav.push(Route::Settings {
                    section: "general".into(),
                });
            }
            Command::PlayPause => ctl::toggle_pause(),
            Command::Next => ctl::next(),
            Command::Previous => ctl::previous(),
            Command::Shuffle => ctl::toggle_shuffle(),
            Command::Repeat => ctl::cycle_repeat(),
            Command::Like if has_track => toggle_like_current(),
            Command::Fullscreen if has_track => toggle_fullscreen(),
            Command::Mini => super::window::toggle_mini(window),
            Command::Sleep => *SLEEP_OPEN.write() = true,
            Command::Effects => *FX_OPEN.write() = true,
            Command::Import => *OPEN_IMPORT.write() = true,
            Command::Like | Command::Fullscreen => {}
        },
    }
}

fn close() {
    *PALETTE_OPEN.write() = false;
}

pub fn toggle() {
    let next = !*PALETTE_OPEN.peek();
    *PALETTE_OPEN.write() = next;
}

#[component]
pub fn CommandPalette() -> Element {
    if !*PALETTE_OPEN.read() {
        return rsx! {};
    }
    rsx! { Panel {} }
}

#[component]
fn Panel() -> Element {
    let window = use_window();
    let mut query = use_signal(String::new);
    let mut active = use_signal(|| 0usize);
    use_effect(|| {
        document::eval(&format!(
            "setTimeout(function(){{var e=document.getElementById('{INPUT_ID}');if(e)e.focus();}},20)"
        ));
    });
    let list = items(&query.read());
    let count = list.len();
    let current = active().min(count.saturating_sub(1));
    let w_key = window.clone();
    let list_key = list.clone();
    rsx! {
        div { class: "fixed inset-0 z-50 bg-(--ow-scrim)", onclick: move |_| close() }
        div {
            role: "dialog",
            "aria-modal": "true",
            "aria-label": t("shortcuts.palette"),
            "data-ow-overlay": "dialog",
            class: "ow-menu-in ow-glass fixed left-1/2 top-[14vh] z-50 flex w-[560px] max-w-[calc(100vw-32px)] -translate-x-1/2 flex-col overflow-hidden rounded-xl",
            div { class: "flex items-center gap-3 border-b border-line px-4",
                Icon { name: "command", size: 18, class: "text-muted" }
                input {
                    id: INPUT_ID,
                    r#type: "text",
                    class: "h-12 min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-placeholder",
                    placeholder: t("palette.placeholder"),
                    "aria-label": t("palette.placeholder"),
                    "aria-controls": "ow-palette-list",
                    "aria-activedescendant": "ow-palette-{current}",
                    value: "{query}",
                    oninput: move |e| {
                        query.set(e.value());
                        active.set(0);
                    },
                    onkeydown: move |e| match e.key() {
                        Key::ArrowDown => {
                            e.prevent_default();
                            active.set((current + 1) % count.max(1));
                        }
                        Key::ArrowUp => {
                            e.prevent_default();
                            active.set((current + count.max(1) - 1) % count.max(1));
                        }
                        Key::Enter => {
                            if let Some(item) = list_key.get(current) {
                                run(item, &w_key);
                            }
                        }
                        Key::Escape => {
                            e.stop_propagation();
                            close();
                        }
                        _ => {}
                    },
                }
            }
            ul { id: "ow-palette-list", role: "listbox", class: "max-h-[52vh] overflow-y-auto p-2",
                for (i, item) in list.into_iter().enumerate() {
                    {
                        let (icon, label) = match &item {
                            Item::Run(_, name, icon) => (*icon, name.clone()),
                            Item::Search(q) => ("search", t("palette.search").replace("{q}", q)),
                        };
                        let tone = if i == current { "bg-surface-2 text-fg" } else { "text-fg" };
                        let w = window.clone();
                        rsx! {
                            li {
                                key: "{i}-{label}",
                                id: "ow-palette-{i}",
                                role: "option",
                                "aria-selected": "{i == current}",
                                class: "flex h-10 cursor-pointer items-center gap-3 rounded-md px-3 text-sm {tone} hover:bg-hover",
                                onmouseenter: move |_| active.set(i),
                                onclick: move |_| run(&item, &w),
                                Icon { name: icon, size: 16, class: "text-muted" }
                                span { class: "truncate", "{label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("palette_tests.rs");
}
