//! The import experience: ONE dialog (`ImportDialog`, opened through `OPEN_IMPORT`) for Spotify,
//! YouTube / YouTube Music links and local files, plus drag-and-drop of audio files.

pub mod account;
mod brand;
mod clipboard;
mod dialog;
mod drop;
mod flow;
mod link_step;
mod result_steps;
mod save;
pub mod state;

pub use dialog::ImportDialog;
pub use drop::{handle_drag_over, handle_drop};
