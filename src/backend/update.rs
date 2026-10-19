//! App updates from this repository's GitHub releases. The installed Windows build downloads the
//! new installer (checked against the release's SHA256SUMS) and runs it silently; every other
//! build (portable zip, macOS, Linux) only learns that a newer version exists.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::ytdlp::{client, compare_versions, find_checksum};

pub const RELEASES: &str = "https://github.com/bebeyat/openwave/releases";
const SETUP_ASSET: &str = "OpenWave-windows-x64-setup.exe";
const SUMS_FILE: &str = "SHA256SUMS";
const MAX_SETUP_BYTES: u64 = 200 * 1024 * 1024;
const MAX_SUMS_BYTES: usize = 64 * 1024;
/// Folder under the data directory that holds a downloaded installer.
pub const DIR: &str = "updates";

/// `v1.2.3` (the last segment of a release URL) as `1.2.3`.
pub fn version_of(location: &str) -> Option<String> {
    let v = location
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .strip_prefix('v')?;
    (!v.is_empty()
        && v.len() <= 20
        && v.split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())))
    .then(|| v.to_owned())
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    compare_versions(latest, current) == std::cmp::Ordering::Greater
}

/// Whether this copy was installed by the Windows installer (it can update itself).
pub fn self_installs() -> bool {
    cfg!(windows)
        && std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join("unins000.exe").is_file()))
            .unwrap_or(false)
}

/// The newest published release, read from the redirect of `releases/latest` (no API, no token).
pub async fn latest() -> Result<String, String> {
    let res = client(false)
        .map_err(|e| e.to_string())?
        .get(format!("{RELEASES}/latest"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    res.headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .and_then(version_of)
        .ok_or_else(|| format!("no release ({})", res.status()))
}

async fn get(http: &reqwest::Client, url: &str) -> Result<reqwest::Response, String> {
    let res = http.get(url).send().await.map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(res)
    } else {
        Err(format!("status {}", res.status()))
    }
}

/// Download the installer of `version` into `dir` and verify it. Returns its path.
pub async fn download(version: &str, dir: &Path) -> Result<PathBuf, String> {
    let http = client(true).map_err(|e| e.to_string())?;
    let base = format!("{RELEASES}/download/v{version}");
    let sums = get(&http, &format!("{base}/{SUMS_FILE}"))
        .await?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    if sums.len() > MAX_SUMS_BYTES {
        return Err("checksum file too large".into());
    }
    let expected = find_checksum(&sums, SETUP_ASSET).ok_or("no checksum")?;
    // Older downloads are of no use any more.
    let _ = tokio::fs::remove_dir_all(dir).await;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| e.to_string())?;
    let target = dir.join(format!("OpenWave-{version}-setup.exe"));
    let part = target.with_extension("part");
    let mut res = get(&http, &format!("{base}/{SETUP_ASSET}")).await?;
    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        size += chunk.len() as u64;
        if size > MAX_SETUP_BYTES {
            return Err("download too large".into());
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got != expected {
        let _ = tokio::fs::remove_file(&part).await;
        return Err("checksum mismatch".into());
    }
    tokio::fs::rename(&part, &target)
        .await
        .map_err(|e| e.to_string())?;
    Ok(target)
}

/// Start the installer without any window. With `relaunch` it opens OpenWave again when done
/// (see `RelaunchAfterUpdate` in installer/openwave.iss). The caller then exits.
pub fn run_installer(setup: &Path, relaunch: bool) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(setup);
    cmd.args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]);
    if relaunch {
        cmd.arg("/RELAUNCH=1");
    }
    cmd.spawn().map(drop)
}

#[cfg(all(test, private_tests))]
#[path = "update_tests.rs"]
mod tests;
