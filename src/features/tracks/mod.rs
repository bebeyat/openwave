//! The shared song list and everything around it, reused by Home, Search, Artist, Album and
//! (later) Library and playlist pages.
//!
//! Public API: [`TrackList`] (props documented in `list.rs`), [`TrackRow`], [`TrackMenuItems`],
//! [`AddToPlaylistDialog`], [`state`] (reactive library access) and [`actions`] (play/open helpers).

pub mod actions;
pub mod common;
pub mod format;
pub mod hero;
pub mod list;
pub mod menu;
pub mod playlist_dialog;
pub mod row;
pub mod sort;
pub mod state;
pub mod window;

pub use list::TrackList;
