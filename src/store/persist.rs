//! Atomic JSON file persistence in a data directory.
//!
//! Writes go to a temporary sibling, are flushed to disk and renamed over the target, so a crash
//! never leaves a half-written file. Unreadable files are moved aside, never deleted.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Result of reading a JSON document.
#[derive(Debug)]
pub enum Read {
    Missing,
    Value(Value),
    /// The file existed but was not valid JSON; it was moved (or copied) to `backup`. `backup` is
    /// `None` when even that failed: the caller must then treat the store as read-only.
    Corrupt {
        backup: Option<PathBuf>,
    },
    /// The file exists but could not be read (locked by antivirus or a sync client, permissions).
    /// Nothing was touched: the caller must not overwrite it.
    Unreadable {
        error: String,
    },
}

const READ_ATTEMPTS: u32 = 6;
const FIRST_BACKOFF: Duration = Duration::from_millis(20);

/// Errors that usually disappear on their own: sharing violations (32), lock violations (33),
/// access denied while another process scans the file.
pub(crate) fn is_transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::{Interrupted, PermissionDenied, TimedOut, WouldBlock};
    matches!(
        e.kind(),
        PermissionDenied | Interrupted | WouldBlock | TimedOut
    ) || matches!(e.raw_os_error(), Some(32 | 33 | 1224))
}

/// Run `op`, retrying transient failures with a short exponential backoff (about 0.6 s total).
pub(crate) fn retry_transient<T>(mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut delay = FIRST_BACKOFF;
    for attempt in 1.. {
        match op() {
            Err(e) if is_transient(&e) && attempt < READ_ATTEMPTS => {
                std::thread::sleep(delay);
                delay *= 2;
            }
            other => return other,
        }
    }
    unreachable!("the loop returns on the last attempt")
}

pub fn read_json(dir: &Path, name: &str) -> Read {
    let path = dir.join(name);
    let bytes = match retry_transient(|| fs::read(&path)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        Err(e) => {
            return Read::Unreadable {
                error: e.to_string(),
            }
        }
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(v) => Read::Value(v),
        Err(_) => Read::Corrupt {
            backup: backup(&path, "corrupt"),
        },
    }
}

pub(crate) fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

/// Move `path` to `<name>.<label>-<unix ms>` (kept for the user to inspect). If the move is
/// refused the file is copied instead, so the original bytes are preserved either way. Returns
/// the new path, or `None` when neither worked (the original must then be left alone).
pub fn backup(path: &Path, label: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let target = path.with_file_name(format!("{name}.{label}-{}", stamp()));
    if retry_transient(|| fs::rename(path, &target)).is_ok() {
        return Some(target);
    }
    retry_transient(|| fs::copy(path, &target))
        .ok()
        .and_then(|_| fs::remove_file(path).ok().map(|()| target.clone()))
        .or_else(|| target.is_file().then_some(target))
}

/// Copy instead of move (used before best-effort reading of files from a newer version).
pub fn copy_backup(path: &Path, label: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let target = path.with_file_name(format!("{name}.{label}"));
    fs::copy(path, &target).ok().map(|_| target)
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `dir/name` atomically (unique temp file in the same directory, fsync, rename).
pub fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let target = dir.join(name);
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!("{name}.{}-{n}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        retry_transient(|| fs::rename(&tmp, &target))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn temp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ow-persist-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_file_is_reported() {
        assert!(matches!(read_json(&temp_dir(), "a.json"), Read::Missing));
    }

    #[test]
    fn atomic_write_round_trips_and_leaves_no_temp_file() {
        let dir = temp_dir();
        write_atomic(&dir, "a.json", br#"{"x":1}"#).unwrap();
        write_atomic(&dir, "a.json", br#"{"x":2}"#).unwrap();
        let Read::Value(v) = read_json(&dir, "a.json") else {
            panic!("expected value")
        };
        assert_eq!(v["x"], 2);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_file_is_moved_aside_with_original_bytes() {
        let dir = temp_dir();
        fs::write(dir.join("a.json"), b"{not json").unwrap();
        let Read::Corrupt { backup: Some(b) } = read_json(&dir, "a.json") else {
            panic!("expected backup")
        };
        assert!(!dir.join("a.json").exists());
        assert_eq!(fs::read(b).unwrap(), b"{not json");
    }

    #[test]
    fn write_into_missing_directory_creates_it() {
        let dir = temp_dir().join("nested/deeper");
        write_atomic(&dir, "a.json", b"{}").unwrap();
        assert!(dir.join("a.json").is_file());
    }

    #[test]
    fn unreadable_file_is_never_moved_or_called_corrupt() {
        // A directory where the file should be: every read fails, but the entry must survive.
        let dir = temp_dir();
        fs::create_dir_all(dir.join("a.json")).unwrap();
        assert!(matches!(read_json(&dir, "a.json"), Read::Unreadable { .. }));
        assert!(dir.join("a.json").is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn sharing_violation_is_retried_until_the_lock_is_released() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = temp_dir();
        fs::write(dir.join("a.json"), br#"{"x":7}"#).unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(dir.join("a.json"))
            .unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        let Read::Value(v) = read_json(&dir, "a.json") else {
            panic!("a transient lock must not look like corruption")
        };
        assert_eq!(v["x"], 7);
        release.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn permanently_locked_file_is_unreadable_not_corrupt() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = temp_dir();
        fs::write(dir.join("a.json"), br#"{"x":7}"#).unwrap();
        let _held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(dir.join("a.json"))
            .unwrap();
        assert!(matches!(read_json(&dir, "a.json"), Read::Unreadable { .. }));
        assert!(dir.join("a.json").exists());
    }

    #[test]
    fn concurrent_writers_use_distinct_temp_files() {
        let dir = temp_dir();
        let threads: Vec<_> = (0..6)
            .map(|i| {
                let d = dir.clone();
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        write_atomic(&d, "a.json", format!("{{\"x\":{i}}}").as_bytes()).unwrap();
                    }
                })
            })
            .collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        assert!(matches!(read_json(&dir, "a.json"), Read::Value(_)));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }
}
