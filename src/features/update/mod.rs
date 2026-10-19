//! Updates from GitHub releases: [`UpdateWatcher`] checks in the background, [`UpdateButton`]
//! appears in the title bar once a newer version is known, and [`UpdateControls`] shows the same
//! state in Settings > About with a manual check. On the installed Windows build the installer is
//! already downloaded: Update installs and restarts, and closing the app installs it silently.
//! Other builds open the release page.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dioxus::prelude::*;

use crate::backend::{api, update};
use crate::i18n::{t, t_args};
use crate::store;
use crate::ui::{Button, ButtonVariant, Icon};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, PartialEq)]
struct Ready {
    version: String,
    /// The verified installer (installed Windows build); `None` elsewhere.
    setup: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Idle,
    Checking,
    UpToDate,
    Ready(Ready),
    Failed,
}

static STATUS: GlobalSignal<Status> = Signal::global(|| Status::Idle);
static INSTALLING: AtomicBool = AtomicBool::new(false);

async fn check() -> Status {
    let Some(version) = api::update_latest().await else {
        return Status::Failed;
    };
    if !update::is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return Status::UpToDate;
    }
    let setup = if update::self_installs() {
        match api::update_download(version.clone()).await {
            Some(path) => Some(path),
            None => return Status::Failed,
        }
    } else {
        None
    };
    Status::Ready(Ready { version, setup })
}

/// Check now unless a check is running or an update is already waiting.
fn check_now() {
    if matches!(*STATUS.peek(), Status::Checking | Status::Ready(_)) {
        return;
    }
    *STATUS.write() = Status::Checking;
    spawn(async {
        let status = check().await;
        *STATUS.write() = status;
    });
}

#[component]
pub fn UpdateWatcher() -> Element {
    use_future(|| async {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if store::settings().get().auto_update {
                check_now();
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
    if let Status::Ready(Ready {
        setup: Some(setup), ..
    }) = STATUS.peek().clone()
    {
        start_installer(&setup, false);
    }
}

/// Install now and restart, or open the release page where the app cannot install itself.
fn apply(ready: &Ready) {
    match &ready.setup {
        Some(setup) => {
            crate::player::session::save_before_exit();
            if start_installer(setup, true) {
                std::process::exit(0);
            }
        }
        None => {
            let _ = open::that(format!("{}/latest", update::RELEASES));
        }
    }
}

#[component]
pub fn UpdateButton() -> Element {
    let Status::Ready(ready) = STATUS() else {
        return rsx! {};
    };
    let title = format!("OpenWave {}", ready.version);
    rsx! {
        button {
            r#type: "button",
            class: "ow-no-drag ow-accent-fill mr-1 inline-flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-bold",
            title: "{title}",
            onclick: move |_| apply(&ready),
            Icon { name: "download", size: 14 }
            {t("update.button")}
        }
    }
}

/// Settings > About: the update state of OpenWave itself, with a manual check.
#[component]
pub fn UpdateControls() -> Element {
    let status = STATUS();
    let note = match &status {
        Status::Idle => None,
        Status::Checking => Some(t("engine.checking")),
        Status::UpToDate => Some(t("engine.up_to_date")),
        Status::Ready(r) => Some(t_args(
            "engine.update_available",
            &[("version", &r.version)],
        )),
        Status::Failed => Some(t("error.network")),
    };
    rsx! {
        div { class: "flex flex-wrap items-center justify-end gap-3",
            if let Some(n) = note {
                span { class: "text-sm text-muted", role: "status", "{n}" }
            }
            if let Status::Ready(ready) = status.clone() {
                Button { icon: "download", onclick: move |_| apply(&ready), {t("update.button")} }
            } else {
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "refresh-cw",
                    loading: status == Status::Checking,
                    onclick: move |_| check_now(),
                    {t("engine.check_update")}
                }
            }
        }
    }
}
