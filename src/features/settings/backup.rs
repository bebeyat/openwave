//! Backup card (Ayarlar › Depolama): save settings and library to one file, or load one back.

use dioxus::prelude::*;

use super::rows::{Card, Row};
use crate::features::tracks::state::update_library;
use crate::i18n::t;
use crate::store::backup::{self, BackupError};
use crate::store::{self, Library};
use crate::ui::{toast_error, toast_success, Button, ButtonVariant};

fn now_ms() -> u64 {
    crate::store::library::now_ms()
}

async fn export() -> Result<bool, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name("openwave-backup.json")
        .add_filter("JSON", &["json"])
        .save_file()
        .await
    else {
        return Ok(false);
    };
    store::flush_all();
    let doc = backup::to_backup(&store::settings().get(), &store::library().get(), now_ms());
    let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
    tokio::fs::write(file.path(), bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

fn error_text(e: &BackupError) -> String {
    match e {
        BackupError::NotABackup => t("backup.error.not_backup"),
        BackupError::Newer => t("backup.error.newer"),
        BackupError::Damaged => t("backup.error.damaged"),
    }
}

async fn import() -> Result<bool, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
        .await
    else {
        return Ok(false);
    };
    let size = tokio::fs::metadata(file.path())
        .await
        .map_err(|e| e.to_string())?
        .len();
    if size > backup::MAX_BYTES {
        return Err(error_text(&BackupError::NotABackup));
    }
    let bytes = tokio::fs::read(file.path())
        .await
        .map_err(|e| e.to_string())?;
    let (settings, library) = backup::from_backup(&bytes).map_err(|e| error_text(&e))?;
    if store::settings().update(|_| settings).is_err() {
        return Err(t("error.store.write"));
    }
    // The queue that is playing now stays; everything else comes from the backup.
    let queue = store::library().get().last_queue.clone();
    update_library(|_| Library {
        last_queue: queue,
        ..library
    });
    super::boot::apply_stored();
    let now = store::settings().get();
    crate::player::effects::update(|fx| fx.with_saved(&now));
    Ok(true)
}

#[component]
pub fn BackupCard() -> Element {
    rsx! {
        Card { class: "mt-5",
            Row { label: t("backup.title"), help: t("backup.help"),
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "upload",
                    onclick: move |_| {
                        spawn(async {
                            match export().await {
                                Ok(true) => toast_success(t("backup.saved")),
                                Ok(false) => {}
                                Err(e) => toast_error(e),
                            }
                        });
                    },
                    {t("backup.export")}
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "download",
                    onclick: move |_| {
                        spawn(async {
                            match import().await {
                                Ok(true) => toast_success(t("backup.restored")),
                                Ok(false) => {}
                                Err(e) => toast_error(e),
                            }
                        });
                    },
                    {t("backup.import")}
                }
            }
        }
    }
}
