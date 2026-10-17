//! Extension points of the shell. Other milestones replace the *body* of these components;
//! the shell keeps their size and position stable.

use dioxus::prelude::*;

/// Set to `true` by any entry point (sidebar "+", Home card, Library "+ Ekle") to open the one
/// import dialog; the import milestone owns the dialog and resets it on close.
pub static OPEN_IMPORT: GlobalSignal<bool> = Signal::global(|| false);

/// A page sets this to `true` while it wants the 320px context panel (artist info, top songs, up
/// next) and back to `false` when it unmounts. The panel only shows at window widths >= 1280px.
pub static RIGHT_PANEL: GlobalSignal<bool> = Signal::global(|| false);

/// The floating 88px glass bar at the bottom. The player milestone fills this in (transport, seek,
/// volume...). Keep the outer element's height (`h-[88px]`) so the content area never jumps.
#[component]
pub fn PlayerBarSlot() -> Element {
    rsx! {
        crate::features::player::PlayerRoot {}
    }
}

/// The compact "Çalma listelerim" list under the sidebar navigation (cover thumb, name, "N şarkı").
#[component]
pub fn SidebarPlaylistsSlot() -> Element {
    rsx! {
        crate::features::library::SidebarPlaylists {}
    }
}

/// Body of the optional right panel; replace with the real content. Rendered inside a glass
/// `aside` that is 320px wide, scrolls on its own and is hidden below 1280px.
#[component]
pub fn RightPanelBody() -> Element {
    rsx! {}
}

/// The right panel frame: nothing is rendered unless [`RIGHT_PANEL`] is on.
#[component]
pub fn RightPanelSlot() -> Element {
    // The queue / lyrics panel takes the column over while it is open on a wide window.
    if crate::features::player::docked_panel_open() {
        return rsx! { crate::features::player::DockedPanel {} };
    }
    if !RIGHT_PANEL() {
        return rsx! {};
    }
    rsx! {
        aside {
            "data-slot": "right-panel",
            class: "ow-panel hidden w-[320px] flex-none overflow-y-auto rounded-xl min-[1280px]:block",
            RightPanelBody {}
        }
    }
}
