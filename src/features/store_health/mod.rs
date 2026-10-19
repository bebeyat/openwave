//! A calm banner shown while a data file cannot be read (antivirus or sync client holding it):
//! the app keeps working with empty data, nothing is saved, and nothing is overwritten. A library
//! file that was damaged (and set aside) is reported once, so an empty library is explained.

use dioxus::prelude::*;
use std::time::Duration;

use crate::i18n::{t, t_args};
use crate::store;
use crate::ui::{Banner, BannerKind, Button, ButtonVariant};

const POLL: Duration = Duration::from_secs(5);

#[component]
pub fn StoreHealthBanner() -> Element {
    let mut read_only = use_signal(|| store::health().read_only);
    let mut recovered = use_signal(|| store::health().recovered);
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
    if read_only() {
        return rsx! {
            div { class: "flex-none px-4 pt-2",
                Banner { kind: BannerKind::Warning, {t("store.health.read_only")} }
            }
        };
    }
    let Some(path) = recovered() else {
        return rsx! {};
    };
    let file = path.display().to_string();
    rsx! {
        div { class: "flex-none px-4 pt-2",
            Banner {
                kind: BannerKind::Warning,
                action: rsx! {
                    Button { variant: ButtonVariant::Ghost, onclick: move |_| recovered.set(None), {t("common.ok")} }
                },
                {t_args("store.health.recovered", &[("file", &file)])}
            }
        }
    }
}
