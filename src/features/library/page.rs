//! `/library/:tab`: Çalma listeleri | Beğenilenler | Geçmiş | Bilgisayarım.

use dioxus::prelude::*;

use super::actions::{add_local_files, add_local_folder, after_menu_closes, open_import};
use super::downloads_tab::DownloadsTab;
use super::logic::normalize_tab;
use super::new_dialog::open_new_playlist;
use super::playlists_tab::PlaylistsTab;
use super::song_tabs::{HistoryTab, LikedTab, LocalTab};
use super::tabs::LibraryTabs;
use crate::features::shell::pages::Page;
use crate::i18n::t;
use crate::routes::Route;
use crate::ui::{ButtonVariant, DropdownMenu, MenuAlign, MenuItem, TabItem, TabPanel};

const GROUP: &str = "lib";

#[component]
fn AddMenu() -> Element {
    rsx! {
        DropdownMenu { label: t("library.add"), variant: ButtonVariant::Primary, align: MenuAlign::End,
            MenuItem {
                icon: "list-plus",
                label: t("library.add.new_playlist"),
                onselect: move |()| after_menu_closes(open_new_playlist),
            }
            MenuItem {
                icon: "download",
                label: t("library.add.import"),
                onselect: move |()| after_menu_closes(open_import),
            }
            MenuItem {
                icon: "hard-drive",
                label: t("library.add.local"),
                onselect: move |()| add_local_files(),
            }
            MenuItem {
                icon: "folder-open",
                label: t("library.add.local_folder"),
                onselect: move |()| add_local_folder(),
            }
        }
    }
}

#[component]
pub fn Library(tab: String) -> Element {
    let nav = navigator();
    let current = normalize_tab(&tab);
    let items = vec![
        TabItem::new("playlists", t("library.tab.playlists")),
        TabItem::new("liked", t("library.tab.liked")),
        TabItem::new("downloads", t("library.tab.downloads")),
        TabItem::new("history", t("library.tab.history")),
        TabItem::new("local", t("library.tab.local")),
    ];
    rsx! {
        Page { title: t("library.title"), actions: Some(rsx! { AddMenu {} }),
            LibraryTabs {
                items,
                value: current,
                group: GROUP,
                aria_label: t("library.title"),
                class: "mb-7",
                on_change: move |v: String| {
                    nav.replace(Route::Library { tab: v });
                },
            }
            {
                rsx! {
                    TabPanel { key: "{current}", value: current, group: GROUP, class: "ow-enter",
                        match current {
                            "liked" => rsx! { LikedTab {} },
                            "downloads" => rsx! { DownloadsTab {} },
                            "history" => rsx! { HistoryTab {} },
                            "local" => rsx! { LocalTab {} },
                            _ => rsx! { PlaylistsTab {} },
                        }
                    }
                }
            }
        }
    }
}
