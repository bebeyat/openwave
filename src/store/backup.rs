//! One-file backup of the settings and the library (playlists, likes, history, local file list).
//! Reading a backup goes through the same migrations and checks as the files on disk.

use serde_json::{json, Value};

use super::versioned::{decode, encode, LoadReport};
use super::{Library, Settings};

pub const FORMAT: &str = "openwave-backup";
const VERSION: u64 = 1;
/// Larger files are not backups of this app (a big library is a few MB).
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    NotABackup,
    Newer,
    Damaged,
}

fn doc<T: super::versioned::Versioned>(value: &T) -> Value {
    encode(value)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}

pub fn to_backup(settings: &Settings, library: &Library, now_ms: u64) -> Value {
    json!({
        "format": FORMAT,
        "version": VERSION,
        "exported_ms": now_ms,
        "settings": doc(settings),
        "library": doc(library),
    })
}

pub fn from_backup(bytes: &[u8]) -> Result<(Settings, Library), BackupError> {
    let root: Value = serde_json::from_slice(bytes).map_err(|_| BackupError::NotABackup)?;
    if root.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(BackupError::NotABackup);
    }
    if root.get("version").and_then(Value::as_u64).unwrap_or(0) > VERSION {
        return Err(BackupError::Newer);
    }
    let mut report = LoadReport::default();
    let mut dropped = Vec::new();
    let part = |key: &str| root.get(key).cloned().ok_or(BackupError::Damaged);
    let settings = decode::<Settings>(part("settings")?, &mut report, &mut dropped)
        .map_err(|()| BackupError::Damaged)?;
    let library = decode::<Library>(part("library")?, &mut report, &mut dropped)
        .map_err(|()| BackupError::Damaged)?;
    Ok((settings, library))
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("backup_tests.rs");
}
