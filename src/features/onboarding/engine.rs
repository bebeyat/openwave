//! State and actions of the audio engine (yt-dlp). Shared by the startup banner and the
//! Hakkında section. Nothing is downloaded until the user presses the install button.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dioxus::dioxus_core::spawn_forever;
use dioxus::prelude::*;

use crate::backend::api;
use crate::backend::error::Coded;
use crate::backend::ytdlp::{self, Phase, Progress, Status};
use crate::i18n::t;

#[derive(Clone, Debug, PartialEq)]
pub enum Engine {
    /// The startup check has not finished.
    Checking,
    Ready {
        version: String,
        /// True when OpenWave downloaded it (as opposed to one found on PATH).
        managed: bool,
    },
    Missing,
    Installing {
        phase: Phase,
        downloaded: u64,
        total: Option<u64>,
    },
    Failed {
        message: String,
    },
}

pub static ENGINE: GlobalSignal<Engine> = Signal::global(|| Engine::Checking);
/// "Şimdi değil": hides the banner until the next start.
pub static BANNER_DISMISSED: GlobalSignal<bool> = Signal::global(|| false);

const POLL: Duration = Duration::from_millis(120);

/// Map a status probe to a state. A binary that does not answer `--version` counts as broken.
pub fn engine_from_status(status: &Status) -> Engine {
    match (&status.located, &status.version) {
        (Some(l), Some(v)) => Engine::Ready {
            version: v.clone(),
            managed: l.origin == ytdlp::Origin::DataDir,
        },
        (Some(_), None) => Engine::Failed {
            message: t("ytdlp.error.broken"),
        },
        (None, _) => Engine::Missing,
    }
}

/// Percent for the progress bar, `None` while the size is unknown.
pub fn percent(downloaded: u64, total: Option<u64>) -> Option<f64> {
    let total = total.filter(|t| *t > 0)?;
    Some((downloaded as f64 * 100.0 / total as f64).clamp(0.0, 100.0))
}

/// Re-check where yt-dlp is. Safe to call at any time (does nothing while installing).
pub fn refresh() {
    spawn_forever(async move {
        if matches!(*ENGINE.peek(), Engine::Installing { .. }) {
            return;
        }
        let status = api::ytdlp_status().await;
        *ENGINE.write() = engine_from_status(&status);
    });
}

/// Download and install the official release. Call only from a click.
pub fn install() {
    if matches!(*ENGINE.peek(), Engine::Installing { .. }) {
        return;
    }
    *ENGINE.write() = Engine::Installing {
        phase: Phase::Downloading,
        downloaded: 0,
        total: None,
    };
    spawn_forever(async move {
        let shared: Arc<Mutex<Option<Progress>>> = Arc::new(Mutex::new(None));
        let writer = shared.clone();
        let job = api::ytdlp_install(move |p| {
            if let Ok(mut slot) = writer.lock() {
                *slot = Some(p);
            }
        });
        let mut job = std::pin::pin!(job);
        let result = loop {
            tokio::select! {
                r = &mut job => break r,
                _ = tokio::time::sleep(POLL) => {
                    let latest = shared.lock().ok().and_then(|s| *s);
                    if let Some(p) = latest {
                        *ENGINE.write() = Engine::Installing {
                            phase: p.phase,
                            downloaded: p.downloaded,
                            total: p.total,
                        };
                    }
                }
            }
        };
        *ENGINE.write() = match result {
            Ok(done) => Engine::Ready {
                version: done.version,
                managed: true,
            },
            Err(e) => Engine::Failed {
                message: e.user_message(),
            },
        };
    });
}

/// Copy a chosen yt-dlp executable into OpenWave's own folder. The caller verifies that it runs.
pub fn copy_into(src: &Path, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(ytdlp::binary_name());
    if src != dest {
        std::fs::copy(src, &dest)?;
    }
    Ok(dest)
}

/// "Elle seç": let the user point at an existing yt-dlp.exe.
pub fn pick_manual() {
    spawn_forever(async move {
        let picked = rfd::AsyncFileDialog::new()
            .add_filter("yt-dlp", &["exe"])
            .pick_file()
            .await;
        let Some(file) = picked else { return };
        let dir = ytdlp::managed_dir();
        if copy_into(file.path(), &dir).is_err() {
            *ENGINE.write() = Engine::Failed {
                message: t("error.io"),
            };
            return;
        }
        let next = engine_from_status(&api::ytdlp_status().await);
        if !matches!(next, Engine::Ready { .. }) {
            // The file did not behave like yt-dlp: do not keep it.
            let _ = std::fs::remove_file(dir.join(ytdlp::binary_name()));
        }
        *ENGINE.write() = next;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::ytdlp::{Located, Origin};

    fn located(origin: Origin) -> Option<Located> {
        Some(Located {
            path: "yt-dlp.exe".into(),
            origin,
        })
    }

    #[test]
    fn status_maps_to_the_states() {
        assert_eq!(engine_from_status(&Status::default()), Engine::Missing);
        let ready = engine_from_status(&Status {
            located: located(Origin::DataDir),
            version: Some("2025.01.01".into()),
        });
        assert_eq!(
            ready,
            Engine::Ready {
                version: "2025.01.01".into(),
                managed: true
            }
        );
        assert!(matches!(
            engine_from_status(&Status {
                located: located(Origin::Path),
                version: Some("1".into())
            }),
            Engine::Ready { managed: false, .. }
        ));
        assert!(matches!(
            engine_from_status(&Status {
                located: located(Origin::Path),
                version: None
            }),
            Engine::Failed { .. }
        ));
    }

    #[test]
    fn percent_handles_unknown_and_overflow() {
        assert_eq!(percent(5, None), None);
        assert_eq!(percent(5, Some(0)), None);
        assert_eq!(percent(50, Some(200)), Some(25.0));
        assert_eq!(percent(500, Some(200)), Some(100.0));
    }

    #[test]
    fn manual_pick_is_copied_into_the_managed_folder() {
        let base = std::env::temp_dir().join(format!("ow-engine-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("tmp");
        let src = base.join("picked.exe");
        std::fs::write(&src, b"x").expect("write");
        let dest = copy_into(&src, &base.join("bin")).expect("copy");
        assert!(dest.is_file());
        assert_eq!(
            dest.file_name().and_then(|n| n.to_str()),
            Some(ytdlp::binary_name())
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
