//! Downloading and installing the official yt-dlp release (checksum verified, atomic replace).
use super::{
    asset_name, binary_name, client, find_checksum, hex, latest_tag, managed_dir, net, run_version,
    valid_version, Installed, Phase, Progress, MAX_BINARY_BYTES, MAX_SUMS_BYTES, RELEASES_BASE,
    SUMS_FILE,
};
use crate::backend::error::YtDlpError;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tokio::io::AsyncWriteExt;

/// Download the newest release, verify it against the published SHA-256 list, check that it runs
/// and install it into [`managed_dir`] (replacing any previous copy atomically).
pub async fn install_latest<F>(on_progress: F) -> Result<Installed, YtDlpError>
where
    F: Fn(Progress) + Send + Sync,
{
    static BUSY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one_at_a_time = BUSY.lock().await;
    let tag = latest_tag().await?;
    install_from(RELEASES_BASE, &tag, &managed_dir(), true, &on_progress).await
}

pub(crate) async fn install_from(
    base: &str,
    tag: &str,
    dir: &Path,
    run_check: bool,
    on_progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<Installed, YtDlpError> {
    if !valid_version(tag) {
        return Err(YtDlpError::BadRelease("bad tag".into()));
    }
    let http = client(true)?;
    let sums = fetch_text(&http, &format!("{base}/download/{tag}/{SUMS_FILE}")).await?;
    let expected = find_checksum(&sums, asset_name()).ok_or(YtDlpError::ChecksumMissing)?;
    tokio::fs::create_dir_all(dir).await?;
    let part = dir.join(format!("{}.part", binary_name()));
    let result = download_verified(&http, base, tag, &part, &expected, on_progress).await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    on_progress(Progress {
        phase: Phase::Installing,
        downloaded: 0,
        total: None,
    });
    let staged = dir.join(format!("new-{}", binary_name()));
    tokio::fs::rename(&part, &staged).await?;
    let version = if run_check {
        run_version(&staged, Duration::from_secs(30)).await
    } else {
        Ok(tag.to_owned())
    };
    let version = match version {
        Ok(v) => v,
        Err(e) => {
            let _ = tokio::fs::remove_file(&staged).await;
            return Err(e);
        }
    };
    let target = dir.join(binary_name());
    replace_file(&staged, &target).await?;
    on_progress(Progress {
        phase: Phase::Done,
        downloaded: 0,
        total: None,
    });
    Ok(Installed {
        path: target,
        version,
    })
}

async fn fetch_text(http: &reqwest::Client, url: &str) -> Result<String, YtDlpError> {
    let res = http.get(url).send().await.map_err(net)?;
    if !res.status().is_success() {
        return Err(YtDlpError::BadRelease(format!("status {}", res.status())));
    }
    let bytes = res.bytes().await.map_err(net)?;
    if bytes.len() > MAX_SUMS_BYTES {
        return Err(YtDlpError::BadRelease("checksum list too large".into()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn download_verified(
    http: &reqwest::Client,
    base: &str,
    tag: &str,
    part: &Path,
    expected: &str,
    on_progress: &(dyn Fn(Progress) + Send + Sync),
) -> Result<(), YtDlpError> {
    let mut res = http
        .get(format!("{base}/download/{tag}/{}", asset_name()))
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(YtDlpError::BadRelease(format!("status {}", res.status())));
    }
    let total = res.content_length();
    let mut file = tokio::fs::File::create(part).await?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    while let Some(chunk) = res.chunk().await.map_err(net)? {
        done += chunk.len() as u64;
        if done > MAX_BINARY_BYTES {
            return Err(YtDlpError::BadRelease("download too large".into()));
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        on_progress(Progress {
            phase: Phase::Downloading,
            downloaded: done,
            total,
        });
    }
    file.sync_all().await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(part, std::fs::Permissions::from_mode(0o755)).await?;
    }
    on_progress(Progress {
        phase: Phase::Verifying,
        downloaded: done,
        total,
    });
    if hex(&hasher.finalize()) == expected {
        Ok(())
    } else {
        Err(YtDlpError::ChecksumMismatch)
    }
}

/// Install a yt-dlp the user picked: copy it next to the managed copy, check that it runs, then
/// swap it in. A file that does not run leaves the current copy untouched.
pub async fn adopt(src: &Path) -> Result<Installed, YtDlpError> {
    adopt_into(src, &managed_dir()).await
}

pub(crate) async fn adopt_into(src: &Path, dir: &Path) -> Result<Installed, YtDlpError> {
    tokio::fs::create_dir_all(dir).await?;
    let target = dir.join(binary_name());
    if src == target {
        let version = run_version(&target, Duration::from_secs(30)).await?;
        return Ok(Installed {
            path: target,
            version,
        });
    }
    let staged = dir.join(format!("new-{}", binary_name()));
    tokio::fs::copy(src, &staged).await?;
    let version = match run_version(&staged, Duration::from_secs(30)).await {
        Ok(v) => v,
        Err(e) => {
            let _ = tokio::fs::remove_file(&staged).await;
            return Err(e);
        }
    };
    replace_file(&staged, &target).await?;
    Ok(Installed {
        path: target,
        version,
    })
}

/// Move `staged` over `target`. A running executable cannot be overwritten on Windows but can be
/// renamed, so the old copy is moved aside first when a direct rename fails.
async fn replace_file(staged: &Path, target: &Path) -> Result<(), YtDlpError> {
    if tokio::fs::rename(staged, target).await.is_ok() {
        return Ok(());
    }
    let old = target.with_extension("old");
    let _ = tokio::fs::remove_file(&old).await;
    tokio::fs::rename(target, &old).await?;
    if let Err(e) = tokio::fs::rename(staged, target).await {
        let _ = tokio::fs::rename(&old, target).await;
        return Err(e.into());
    }
    let _ = tokio::fs::remove_file(&old).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_picked_file_that_does_not_run_keeps_the_current_copy() {
        let dir = std::env::temp_dir().join(format!("ow-adopt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("tmp");
        std::fs::write(dir.join(binary_name()), b"old").expect("old");
        let picked = dir.join("picked.bin");
        std::fs::write(&picked, b"not an executable").expect("picked");
        let err = adopt_into(&picked, &dir).await.unwrap_err();
        assert_eq!(err, YtDlpError::BrokenBinary);
        assert_eq!(
            std::fs::read(dir.join(binary_name())).expect("read"),
            b"old"
        );
        assert!(!dir.join(format!("new-{}", binary_name())).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn release_asset_is_a_standalone_build_for_this_os() {
        // The bare `yt-dlp` asset is a Python zipapp; never download it.
        assert!(asset_name().starts_with("yt-dlp") && asset_name() != "yt-dlp");
    }
}
