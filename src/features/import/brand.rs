//! Source logos for the import picker. Spotify and YouTube marks are the Simple Icons paths
//! (CC0 1.0, simple-icons 13.21.0), drawn in each brand's own colour to name the source.

use dioxus::prelude::*;

use crate::ui::Icon;

const SPOTIFY_PATH: &str = "M12 0C5.4 0 0 5.4 0 12s5.4 12 12 12 12-5.4 12-12S18.66 0 12 0zm5.521 17.34c-.24.359-.66.48-1.021.24-2.82-1.74-6.36-2.101-10.561-1.141-.418.122-.779-.179-.899-.539-.12-.421.18-.78.54-.9 4.56-1.021 8.52-.6 11.64 1.32.42.18.479.659.301 1.02zm1.44-3.3c-.301.42-.841.6-1.262.3-3.239-1.98-8.159-2.58-11.939-1.38-.479.12-1.02-.12-1.14-.6-.12-.48.12-1.021.6-1.141C9.6 9.9 15 10.561 18.72 12.84c.361.181.54.78.241 1.2zm.12-3.36C15.24 8.4 8.82 8.16 5.16 9.301c-.6.179-1.2-.181-1.38-.721-.18-.601.18-1.2.72-1.381 4.26-1.26 11.28-1.02 15.721 1.621.539.3.719 1.02.419 1.56-.299.421-1.02.599-1.559.3z";
const YOUTUBE_PATH: &str = "M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brand {
    Spotify,
    Youtube,
    Apple,
    Local,
}

#[component]
pub fn BrandLogo(brand: Brand, #[props(default = 44)] size: u32) -> Element {
    match brand {
        Brand::Spotify => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                class: "flex-none",
                "aria-hidden": "true",
                "focusable": "false",
                // The waves are cut out of the disc; a dark backing keeps them dark on light glass.
                circle { cx: "12", cy: "12", r: "11", fill: "#0c0b0a" }
                path { d: SPOTIFY_PATH, fill: "#1ed760" }
            }
        },
        Brand::Youtube => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                class: "flex-none",
                "aria-hidden": "true",
                "focusable": "false",
                // The play triangle is a hole in the mark; white behind it, as in the real logo.
                rect { x: "8", y: "7.5", width: "9", height: "9", fill: "#ffffff" }
                path { d: YOUTUBE_PATH, fill: "#ff0033" }
            }
        },
        Brand::Apple => rsx! {
            svg {
                width: "{size}",
                height: "{size}",
                view_box: "0 0 24 24",
                class: "flex-none",
                "aria-hidden": "true",
                "focusable": "false",
                // A plain note on Apple Music's red tile.
                rect { x: "1", y: "1", width: "22", height: "22", rx: "5.5", fill: "#fa2d48" }
                path {
                    d: "M16.5 5.2v9.3a2.3 2.3 0 1 1-1.3-2.07V8.4l-5.4 1.08v6.52a2.3 2.3 0 1 1-1.3-2.07V7.4z",
                    fill: "#ffffff",
                }
            }
        },
        Brand::Local => rsx! {
            Icon { name: "folder-open", size, class: "text-[#f5b93a]" }
        },
    }
}
