//! Native services: catalog, media proxy, import, lyrics, yt-dlp bootstrap.
//! Entry points for the UI are in [`api`]; typed errors are in [`error`].
// The UI consumes this API incrementally; unused items are expected until it does.

pub mod api;
pub mod catalog;
mod convert;
pub mod error;
pub mod import;
pub mod import_flow;
mod innertube;
pub mod local_tags;
pub mod lyrics;
pub mod lyrics_cache;
pub mod lyrics_ytm;
pub mod media;
pub mod palette;
mod palette_net;
mod proxy;
pub mod share;
pub mod soundcloud;
pub mod spotify_account;
pub mod spotify_auth;
pub mod translate;
pub mod update;
pub mod ytdlp;

pub use api::{data_dir, init};
pub use error::{ApiError, Coded};
