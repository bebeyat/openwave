//! yt-dlp bootstrap: find it, report its state, download the official release with checksum
//! verification, install atomically, check for updates.
//!
//! Nothing here runs by itself. [`install_latest`] and [`check_update`] touch the network only
//! when the UI calls them (after the user agreed). Hosts used: `github.com` and the two exact
//! hosts GitHub redirects release assets to (see [`allowed_host`]).
//!
//! Trust: the managed copy in the data directory is preferred. A copy found on `PATH` is
//! untrusted: only absolute `PATH` entries count (never the current directory or an empty entry),
//! it is never executed just to read its version, and it only runs when a stream fallback needs it.
use super::error::YtDlpError;
use std::{
    cmp::Ordering,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

pub const RELEASES_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases";
const SUMS_FILE: &str = "SHA2-256SUMS";
const MAX_BINARY_BYTES: u64 = 120 * 1024 * 1024;
const MAX_SUMS_BYTES: usize = 256 * 1024;

pub fn binary_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

/// The release asset to download: a standalone build for this OS. The plain `yt-dlp` asset is a
/// Python zipapp, which a stock macOS or Linux desktop cannot run.
pub fn asset_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else if cfg!(target_os = "macos") {
        "yt-dlp_macos"
    } else if cfg!(target_arch = "aarch64") {
        "yt-dlp_linux_aarch64"
    } else {
        "yt-dlp_linux"
    }
}

/// `<data dir>/bin`, where the downloaded copy lives.
pub fn managed_dir() -> PathBuf {
    super::api::data_dir().join("bin")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The copy OpenWave downloaded.
    DataDir,
    /// Found on the system `PATH`.
    Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    pub path: PathBuf,
    pub origin: Origin,
}

/// Data directory first, then `PATH`.
pub fn locate() -> Option<Located> {
    locate_in(&managed_dir(), std::env::var_os("PATH"))
}

/// Only absolute, non-empty `PATH` entries are searched: a relative entry (`.`, `bin`, empty)
/// would resolve against the current directory.
pub(crate) fn locate_in(dir: &Path, path_var: Option<OsString>) -> Option<Located> {
    let own = dir.join(binary_name());
    if own.is_file() {
        return Some(Located {
            path: own,
            origin: Origin::DataDir,
        });
    }
    std::env::split_paths(&path_var?)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(binary_name()))
        .find(|p| p.is_file())
        .map(|path| Located {
            path,
            origin: Origin::Path,
        })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub located: Option<Located>,
    /// `None` when missing or when the managed binary did not answer `--version`. A copy found on
    /// `PATH` is not executed to ask: it reports [`UNVERIFIED_VERSION`].
    pub version: Option<String>,
}

/// Reported instead of a version for a yt-dlp found on `PATH` (it is not run at startup).
pub const UNVERIFIED_VERSION: &str = "unverified";

/// Where yt-dlp is and which version it reports. Only the managed copy is executed.
pub async fn status() -> Status {
    status_of(locate()).await
}

pub(crate) async fn status_of(located: Option<Located>) -> Status {
    let version = match &located {
        Some(l) if l.origin == Origin::DataDir => {
            // Generous: a one-file build unpacks itself on every run, and the first run after a
            // download is slowed further by antivirus scans.
            run_version(&l.path, Duration::from_secs(30)).await.ok()
        }
        Some(_) => Some(UNVERIFIED_VERSION.to_owned()),
        None => None,
    };
    Status { located, version }
}

/// A yt-dlp process that cannot pick up files from wherever the app was started: it runs with the
/// managed directory as working directory, without a console window.
pub(crate) fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    let dir = managed_dir();
    let _ = std::fs::create_dir_all(&dir);
    cmd.current_dir(dir).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

async fn run_version(path: &Path, limit: Duration) -> Result<String, YtDlpError> {
    let mut cmd = command(path);
    cmd.arg("--version");
    let out = tokio::time::timeout(limit, cmd.output())
        .await
        .map_err(|_| YtDlpError::Timeout)?
        .map_err(|_| YtDlpError::BrokenBinary)?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if out.status.success() && valid_version(&text) {
        Ok(text)
    } else {
        Err(YtDlpError::BrokenBinary)
    }
}

fn valid_version(v: &str) -> bool {
    !v.is_empty() && v.len() <= 32 && v.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Compare yt-dlp calendar versions such as `2025.09.26` or `2025.10.22.232902`.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |v: &str| {
        v.split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(a).cmp(&parse(b))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Downloading,
    Verifying,
    Installing,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateCheck {
    pub installed: Option<String>,
    pub latest: String,
    pub update_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    pub path: PathBuf,
    pub version: String,
}

fn net(e: reqwest::Error) -> YtDlpError {
    if e.is_timeout() {
        YtDlpError::Timeout
    } else {
        YtDlpError::Network(e.to_string())
    }
}

/// Hosts GitHub release downloads use: the release page itself and the two exact asset hosts it
/// redirects to. No wildcard: any other `*.githubusercontent.com` host serves user content.
const ALLOWED_HOSTS: [&str; 3] = [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

pub(crate) fn allowed_host(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
}

pub(crate) fn client(follow: bool) -> Result<reqwest::Client, YtDlpError> {
    let policy = if follow {
        reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() < 5 && allowed_host(a.url()) {
                a.follow()
            } else {
                a.stop()
            }
        })
    } else {
        reqwest::redirect::Policy::none()
    };
    reqwest::Client::builder()
        .user_agent(concat!("OpenWave/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .redirect(policy)
        .build()
        .map_err(net)
}

/// Last path segment of the redirect target of `<base>/latest`, validated as a version tag.
pub(crate) fn tag_from_location(location: &str) -> Option<String> {
    let tag = location.trim_end_matches('/').rsplit('/').next()?;
    (location.contains("/tag/") && valid_version(tag)).then(|| tag.to_owned())
}

/// Tag of the newest official release (one small request, no API token, no redirect followed).
pub async fn latest_tag() -> Result<String, YtDlpError> {
    latest_tag_from(RELEASES_BASE).await
}

pub(crate) async fn latest_tag_from(base: &str) -> Result<String, YtDlpError> {
    let res = client(false)?
        .get(format!("{base}/latest"))
        .send()
        .await
        .map_err(net)?;
    let location = res
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| YtDlpError::BadRelease(format!("status {}", res.status())))?;
    tag_from_location(location).ok_or_else(|| YtDlpError::BadRelease("unexpected tag".into()))
}

/// Compare the installed version with the newest release. `known` is the version the caller
/// already read; asking yt-dlp again can time out, which used to look like "nothing installed".
pub async fn check_update(known: Option<String>) -> Result<UpdateCheck, YtDlpError> {
    let latest = latest_tag().await?;
    let installed = match known {
        Some(v) => Some(v),
        None => status().await.version,
    }
    .filter(|v| v != UNVERIFIED_VERSION);
    let update_available = installed
        .as_deref()
        .is_none_or(|v| compare_versions(&latest, v) == Ordering::Greater);
    Ok(UpdateCheck {
        installed,
        latest,
        update_available,
    })
}

/// Find `file` in a `SHA2-256SUMS` document (`<hex>  <name>` or `<hex> *<name>`).
pub fn find_checksum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[path = "ytdlp_install.rs"]
mod install;
pub use install::{adopt, install_latest};

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("ytdlp_tests.rs");
}
