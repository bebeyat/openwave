//! `Shell`: the layout every route renders inside. Floating glass panels with 8px gaps over the
//! ambient backdrop, under one top strip on the backdrop itself (no separate title bar panel).
//!
//! ```text
//!  TopBar: wordmark  | search pill ........... window buttons
//! +---------+--------------------------------+---------+
//! | Sidebar |                                |         |
//! | 248px   |                                |         |
//! | (rail   | main panel, radius 24, scrolls | right   |
//! |  76px   | <Outlet/>                      | 320px   |
//! |  <1100) |                                | >=1280  |
//! +---------+--------------------------------+---------+
//! | PlayerBarSlot: floating glass bar, 88px              |
//! +------------------------------------------------------+
//! ```

use dioxus::prelude::*;

use super::shortcuts::{use_shortcut_bridge, use_shortcuts, Action};
use super::sidebar::{Sidebar, Wordmark};
use super::slots::{PlayerBarSlot, RightPanelSlot};
use super::titlebar::{DragRegion, WindowControls};
use crate::i18n::t;
use crate::routes::Route;
use crate::ui::SearchInput;

/// Id shared with `ui::js` so `/` and Ctrl+K can focus the field.
pub const SEARCH_ID: &str = "ow-global-search";

#[component]
fn TopBar() -> Element {
    let route = use_route::<Route>();
    let nav = navigator();
    let mut query = use_signal(String::new);

    // Keep the field in sync when the route (not the user) changes the query.
    use_effect(use_reactive!(|route| {
        if let Route::Search { q } = route {
            if *query.peek() != q {
                query.set(q);
            }
        }
    }));

    // One strip across the window: wordmark (sidebar width) | search (main column start) | drag | buttons.
    rsx! {
        header { class: "flex h-12 flex-none items-center gap-2",
            Wordmark {}
            SearchInput {
                id: SEARCH_ID,
                class: "ow-no-drag w-full max-w-[520px]",
                value: query(),
                aria_label: t("search.aria_label"),
                placeholder: t("search.placeholder"),
                // The search page (recent searches, suggestions) has no sidebar link: focusing the
                // field opens it.
                onfocus: move |_| {
                    if !matches!(router().current::<Route>(), Route::Search { .. }) {
                        nav.push(Route::Search { q: query.peek().clone() });
                    }
                },
                oninput: move |q: String| {
                    query.set(q.clone());
                    let target = Route::Search { q };
                    if matches!(router().current::<Route>(), Route::Search { .. }) {
                        nav.replace(target);
                    } else {
                        nav.push(target);
                    }
                },
            }
            DragRegion { class: "h-full min-w-6 flex-1" }
            crate::features::update::UpdateButton {}
            WindowControls {}
        }
    }
}

#[component]
pub fn Shell() -> Element {
    use_shortcut_bridge();
    let nav = navigator();
    use_shortcuts(move |action| {
        if action == Action::Palette {
            super::palette::toggle();
            return;
        }
        if action == Action::Escape {
            // Close the queue drawer / fullscreen player / shortcut popover first.
            if crate::features::player::state::close_top_overlay() {
                return;
            }
            if router().current::<Route>().is_detail() {
                nav.go_back();
            }
        }
    });
    rsx! {
        div {
            class: "relative z-10 flex h-full flex-col gap-(--ow-gap) p-(--ow-gap) text-fg",
            ondragover: crate::features::import::handle_drag_over,
            ondrop: crate::features::import::handle_drop,
            TopBar {}
            div { class: "flex min-h-0 flex-1 gap-(--ow-gap)",
                Sidebar {}
                div { class: "flex min-w-0 flex-1 flex-col gap-(--ow-gap)",
                    crate::features::store_health::StoreHealthBanner {}
                    div { class: "flex min-h-0 flex-1 gap-(--ow-gap)",
                        main { id: "ow-main", class: "ow-panel min-h-0 min-w-0 flex-1 overflow-y-auto rounded-xl",
                            crate::features::onboarding::Onboarding {}
                            Outlet::<Route> {}
                        }
                        RightPanelSlot {}
                    }
                }
            }
            PlayerBarSlot {}
            crate::features::import::ImportDialog {}
            crate::features::share::ShareDialog {}
            crate::features::share::LinkInbox {}
            crate::features::update::UpdateWatcher {}
            super::window::TrayHost {}
            super::palette::CommandPalette {}
        }
    }
}
