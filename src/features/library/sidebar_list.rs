//! Body of the sidebar slot "Çalma listelerim": the most recently updated playlists, each with a
//! cover thumb (first cover found in the playlist), its name and "N şarkı". In the 76px rail only
//! the thumbs remain (the name is the tooltip and the accessible name).

use dioxus::prelude::*;

use super::logic::recent_playlists;
use super::new_dialog::NewPlaylistDialog;
use crate::features::tracks::state::library;
use crate::i18n::{t, t_count};
use crate::routes::Route;
use crate::ui::CoverThumb;

const MAX_ROWS: usize = 8;

#[component]
pub fn SidebarPlaylists() -> Element {
    let lib = library();
    let route = use_route::<Route>();
    let active = match &route {
        Route::Playlist { id } => Some(id.clone()),
        _ => None,
    };
    let items = recent_playlists(&lib.playlists, MAX_ROWS);
    rsx! {
        if items.is_empty() {
            p { class: "px-3 py-2 text-sm text-muted max-[1099px]:hidden", "data-slot": "sidebar-playlists",
                {t("sidebar.playlists_empty")}
            }
        } else {
            ul { "data-slot": "sidebar-playlists", class: "flex flex-col gap-1",
                for p in items {
                    {
                        let is_active = active.as_deref() == Some(p.id.as_str());
                        let cover = p.cover.clone().filter(|c| !c.is_empty()).or_else(|| {
                            p.tracks
                                .iter()
                                .find_map(|t| t.cover.as_ref().map(|c| c.url.to_string()))
                        });
                        let count = t_count("common.songs", p.tracks.len());
                        rsx! {
                            li { key: "{p.id}",
                                Link {
                                    to: Route::Playlist { id: p.id.to_string() },
                                    title: "{p.name}",
                                    "aria-label": "{p.name}",
                                    "aria-current": is_active.then_some("page"),
                                    class: "ow-trow flex h-14 items-center gap-3 rounded-md px-2 hover:bg-hover max-[1099px]:justify-center max-[1099px]:px-0",
                                    CoverThumb { src: cover, size: 44 }
                                    div { class: "min-w-0 flex-1 max-[1099px]:hidden",
                                        p { class: "truncate text-sm font-semibold text-fg", "{p.name}" }
                                        p { class: "truncate text-xs text-muted", "{count}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        NewPlaylistDialog {}
    }
}
