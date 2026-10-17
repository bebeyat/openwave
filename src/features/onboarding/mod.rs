//! Startup experience: applies the saved settings once, then shows the audio-engine banner
//! when yt-dlp is missing. Mounted once in the app shell as `Onboarding {}`.

mod banner;
pub mod engine;

use dioxus::prelude::*;

pub use banner::EngineBanner;

#[component]
pub fn Onboarding() -> Element {
    use_hook(crate::features::settings::boot::apply_stored);
    rsx! { EngineBanner {} }
}
