//! Left navigation panel: Home and Library (search and the wordmark live in the top strip), the
//! playlists list (cover thumbs),
//! Settings pinned at the bottom. 248px wide, floating glass; below 1100px window width it
//! narrows to a 76px rail that keeps a small text label under every icon (labels are never hidden).

use dioxus::prelude::*;

use super::slots::SidebarPlaylistsSlot;
use super::titlebar::DragRegion;
use crate::i18n::t;
use crate::routes::Route;
use crate::ui::{Emblem, Icon, IconButton, IconButtonSize, TooltipAlign, TooltipSide};

#[derive(Clone, Copy, PartialEq)]
enum NavKind {
    Home,
    Library,
    Settings,
}

impl NavKind {
    fn route(self) -> Route {
        match self {
            NavKind::Home => Route::Home {},
            NavKind::Library => Route::Library {
                tab: "playlists".into(),
            },
            NavKind::Settings => Route::Settings {
                section: "general".into(),
            },
        }
    }

    fn is_active(self, current: &Route) -> bool {
        matches!(
            (self, current),
            (NavKind::Home, Route::Home {})
                | (NavKind::Library, Route::Library { .. })
                | (NavKind::Settings, Route::Settings { .. })
        )
    }
}

#[component]
fn NavLink(kind: NavKind, icon: &'static str, label: String) -> Element {
    let current = use_route::<Route>();
    let active = kind.is_active(&current);
    let tone = if active {
        "bg-surface-2 text-fg font-bold"
    } else {
        "text-muted hover:bg-hover hover:text-fg font-medium"
    };
    let icon_tone = if active { "text-accent-text" } else { "" };
    rsx! {
        Link {
            to: kind.route(),
            "aria-current": active.then_some("page"),
            class: "relative flex h-11 items-center gap-3.5 rounded-md px-4 text-[15px] transition-colors duration-(--ow-dur-hover) max-[1099px]:h-14 max-[1099px]:flex-col max-[1099px]:justify-center max-[1099px]:gap-1 max-[1099px]:px-0 {tone}",
            if active {
                span { class: "ow-accent-fill absolute left-0 top-1/2 h-6 w-1 -translate-y-1/2 rounded-full max-[1099px]:left-1/2 max-[1099px]:top-0 max-[1099px]:h-[3px] max-[1099px]:w-6 max-[1099px]:-translate-x-1/2 max-[1099px]:translate-y-0", "aria-hidden": "true" }
            }
            Icon { name: icon, size: 22, class: "{icon_tone}" }
            span { class: "max-w-full truncate max-[1099px]:text-[11px] max-[1099px]:leading-[12px]", "{label}" }
        }
    }
}

/// Brand mark + name, shown in the top strip above the sidebar (same width, so they line up).
#[component]
pub fn Wordmark() -> Element {
    rsx! {
        div { class: "relative flex h-full w-[248px] flex-none items-center gap-2.5 px-6 max-[1099px]:w-[76px] max-[1099px]:justify-center max-[1099px]:px-0",
            DragRegion { class: "absolute inset-0" }
            Emblem { height: 16, class: "pointer-events-none relative text-accent-text" }
            span { class: "ow-display pointer-events-none relative text-lg font-extrabold tracking-tight text-fg max-[1099px]:sr-only", "OpenWave" }
        }
    }
}

#[component]
pub fn Sidebar() -> Element {
    rsx! {
        aside {
            class: "ow-panel flex w-[248px] flex-none flex-col rounded-xl max-[1099px]:w-[76px]",
            nav {
                "aria-label": t("nav.label"),
                class: "flex flex-col gap-1 px-3 pt-3 max-[1099px]:px-2",
                NavLink { kind: NavKind::Home, icon: "house", label: t("nav.home") }
                NavLink { kind: NavKind::Library, icon: "library", label: t("nav.library") }
            }
            if !crate::features::settings::boot::CUSTOM.read().sidebar_playlists {
                div { class: "flex-1" }
            } else {
            section {
                "aria-label": t("nav.playlists"),
                class: "mt-4 flex min-h-0 flex-1 flex-col px-3 max-[1099px]:px-2",
                div { class: "flex items-center justify-between pl-4 max-[1099px]:justify-center max-[1099px]:pl-0",
                    h2 { class: "font-sans text-[13px] font-semibold tracking-normal text-muted max-[1099px]:sr-only", {t("nav.playlists")} }
                    IconButton {
                        icon: "plus",
                        label: t("sidebar.new_playlist"),
                        size: IconButtonSize::Sm,
                        tooltip_side: TooltipSide::Bottom,
                        tooltip_align: TooltipAlign::End,
                        onclick: move |_| crate::features::library::open_new_playlist(),
                    }
                }
                div { class: "mt-1 min-h-0 flex-1 overflow-y-auto", SidebarPlaylistsSlot {} }
            }
            }
            div { class: "flex flex-col gap-1 px-3 pb-3 pt-2 max-[1099px]:px-2",
                NavLink { kind: NavKind::Settings, icon: "settings", label: t("nav.settings") }
            }
        }
    }
}
