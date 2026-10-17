//! Placeholder pages for every route. Each screen milestone replaces ONE of these functions
//! (same name and props) by pointing the `use` in `routes.rs` at its own module.
//! `Page` and `BackButton` are the shared page chrome for all screens.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::ui::{Button, ButtonSize, ButtonVariant, EmptyState};

/// Standard page container: max width, padding, optional "← Geri" button and an `h1`.
/// The top search field uses the same container width so edges line up.
#[component]
pub fn Page(
    #[props(into)] title: String,
    #[props(default)] back: bool,
    #[props(default)] actions: Option<Element>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "ow-page pb-10 pt-8",
            if back {
                BackButton {}
            }
            div { class: "mb-6 flex min-h-10 items-center justify-between gap-4",
                h1 { class: "ow-display text-3xl font-extrabold tracking-tight text-fg", "{title}" }
                if let Some(a) = actions {
                    div { class: "flex items-center gap-2", {a} }
                }
            }
            {children}
        }
    }
}

/// "← Geri" button for detail pages (Esc also goes back; see `layout.rs`).
#[component]
pub fn BackButton() -> Element {
    let nav = navigator();
    rsx! {
        Button {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Sm,
            icon: "arrow-left",
            class: "-ml-3 mb-2",
            onclick: move |_| nav.go_back(),
            {t("common.back")}
        }
    }
}

#[component]
fn Placeholder(title: String, icon: &'static str, detail: bool) -> Element {
    rsx! {
        Page { title, back: detail,
            EmptyState { icon, title: t("page.placeholder_title"), text: t("page.placeholder_text") }
        }
    }
}

#[component]
pub fn Home() -> Element {
    rsx! { crate::features::home::Home {} }
}

#[component]
pub fn Search(q: String) -> Element {
    rsx! { crate::features::search::Search { q } }
}

#[component]
pub fn Library(tab: String) -> Element {
    rsx! { crate::features::library::Library { tab } }
}

#[component]
pub fn Playlist(id: String) -> Element {
    rsx! { crate::features::library::Playlist { id } }
}

#[component]
pub fn Album(id: String) -> Element {
    rsx! { crate::features::album::Album { id } }
}

#[component]
pub fn Artist(id: String) -> Element {
    rsx! { crate::features::artist::Artist { id } }
}

pub use crate::features::settings::Settings;
