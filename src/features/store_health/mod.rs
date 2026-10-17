//! A calm banner shown while a data file cannot be read (antivirus or sync client holding it):
//! the app keeps working with empty data, nothing is saved, and nothing is overwritten.

use dioxus::prelude::*;
use std::time::Duration;

use crate::i18n::t;
use crate::store;
use crate::ui::{Banner, BannerKind};

const POLL: Duration = Duration::from_secs(5);

#[component]
pub fn StoreHealthBanner() -> Element {
    let mut read_only = use_signal(|| store::health().read_only);
    use_future(move || async move {
        loop {
            tokio::time::sleep(POLL).await;
            // Also re-tries the unreadable file, so the banner clears by itself.
            let now = tokio::task::spawn_blocking(|| store::health().read_only)
                .await
                .unwrap_or(false);
            if *read_only.peek() != now {
                read_only.set(now);
            }
        }
    });
    if !read_only() {
        return rsx! {};
    }
    rsx! {
        div { class: "flex-none px-4 pt-2",
            Banner { kind: BannerKind::Warning, {t("store.health.read_only")} }
        }
    }
}
