//! Application routes (SPEC section 2). All pages render inside the `Shell` layout.
//!
//! To replace a placeholder page, change the component imported below to your own function with the
//! same name and props.

use dioxus::prelude::*;

use crate::features::shell::pages::{Album, Artist, Home, Library, Playlist, Search, Settings};
use crate::features::shell::Shell;

#[derive(Routable, Clone, PartialEq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[redirect("/library", || Route::Library { tab: "playlists".to_string() })]
    #[redirect("/settings", || Route::Settings { section: "general".to_string() })]
    #[layout(Shell)]
        #[route("/")]
        Home {},
        #[route("/search?:q")]
        Search { q: String },
        /// `tab`: playlists | liked | history | local
        #[route("/library/:tab")]
        Library { tab: String },
        #[route("/playlist/:id")]
        Playlist { id: String },
        #[route("/album/:id")]
        Album { id: String },
        #[route("/artist/:id")]
        Artist { id: String },
        /// `section`: general | playback | lyrics | storage | privacy | shortcuts | about
        #[route("/settings/:section")]
        Settings { section: String },
}

impl Route {
    /// Detail pages show a "← Geri" button and respond to Esc.
    pub fn is_detail(&self) -> bool {
        matches!(
            self,
            Route::Playlist { .. } | Route::Album { .. } | Route::Artist { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_routes() {
        assert!(Route::Playlist { id: "a".into() }.is_detail());
        assert!(!Route::Home {}.is_detail());
        assert!(!Route::Search { q: String::new() }.is_detail());
    }

    #[test]
    fn paths_parse() {
        let r: Route = "/library/liked".parse().expect("route");
        assert_eq!(
            r,
            Route::Library {
                tab: "liked".into()
            }
        );
        let r: Route = "/search?q=abc".parse().expect("route");
        assert_eq!(r, Route::Search { q: "abc".into() });
    }
}
