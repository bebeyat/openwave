//! Library screens: `/library/:tab`, `/playlist/:id` and the sidebar playlist list.

pub mod actions;
pub mod cover;
mod downloads_tab;
pub mod logic;
pub mod new_dialog;
pub mod page;
pub mod playlist_page;
pub mod playlists_tab;
pub mod sidebar_list;
pub mod song_tabs;
pub mod tabs;
pub mod title_editor;

pub use new_dialog::open_new_playlist;
pub use page::Library;
pub use playlist_page::Playlist;
pub use sidebar_list::SidebarPlaylists;
