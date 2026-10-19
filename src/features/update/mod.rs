//! Updates from GitHub releases: [`UpdateWatcher`] checks in the background, [`UpdateButton`]
//! appears in the title bar once a newer version is known. On the installed Windows build the
//! installer is already downloaded: the button installs and restarts, and closing the app
//! installs it silently. Other builds open the release page.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dioxus::prelude::*;

use crate::backend::{api, update};
use crate::i18n::t;
use crate::store;
use crate::ui::Icon;

const FIRST_CHECK: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, PartialEq)]
struct Ready {
    version: String,
    /// The verified installer (installed Windows build); `None` elsewhere.
    setup: Option<PathBuf>,
}

static READY: GlobalSignal<Option<Ready>> = Signal::global(|| None);
static INSTALLING: AtomicBool = AtomicBool::new(false);

async fn check() -> Option<Ready> {
    let version = api::update_latest().await?;
    if !update::is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return None;
    }
    let setup = if update::self_installs() {
        Some(api::update_download(version.clone()).await?)
    } else {
        None
    };
    Some(Ready { version, setup })
}

#[component]
pub fn UpdateWatcher() -> Element {
    use_future(|| async {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if store::settings().get().auto_update {
                if let Some(ready) = check().await {
                    *READY.write() = Some(ready);
                    return;
                }
            }
            tokio::time::sleep(EVERY).await;
        }
    });
    rsx! {}
}

fn start_installer(setup: &std::path::Path, relaunch: bool) -> bool {
    !INSTALLING.swap(true, Ordering::SeqCst) && update::run_installer(setup, relaunch).is_ok()
}

/// Called when the window closes: a downloaded update installs while the app is gone.
pub fn install_on_exit() {
    if let Some(Ready {
        setup: Some(setup), ..
    }) = READY.peek().clone()
    {
        start_installer(&setup, false);
    }
}

#[component]
pub fn UpdateButton() -> Element {
    let Some(ready) = READY() else {
        return rsx! {};
    };
    let title = format!("OpenWave {}", ready.version);
    rsx! {
        button {
            r#type: "button",
            class: "ow-no-drag ow-accent-fill mr-1 inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-bold",
            title: "{title}",
            onclick: move |_| match &ready.setup {
                Some(setup) => {
                    crate::player::session::save_before_exit();
                    if start_installer(setup, true) {
                        std::process::exit(0);
                    }
                }
                None => {
                    let _ = open::that(format!("{}/latest", update::RELEASES));
                }
            },
            Icon { name: "download", size: 14 }
            {t("update.button")}
        }
    }
}
