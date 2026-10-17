//! Generic versioned JSON store.
//!
//! A document type implements [`Versioned`]: a file name, the current schema version and a
//! per-step migration on the raw JSON. Files without a `version` field are version 0. Loading
//! never loses data: a file that is not JSON is moved aside, items that cannot be decoded are
//! quarantined one by one, a file written by a newer app version is copied to `<name>.v<N>.bak`,
//! and a file that merely cannot be *read* right now puts the store in a read-only state instead
//! of being replaced by defaults.
use super::persist::{self, Read};
use crate::backend::error::Coded;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub use super::handle::Store;

/// A small extra document written next to the main one (only when its content changes).
pub struct Sidecar {
    pub file: &'static str,
    /// `None` removes the file.
    pub value: Option<Value>,
}

pub trait Versioned:
    Serialize + DeserializeOwned + Default + Clone + Send + Sync + 'static
{
    const FILE: &'static str;
    const VERSION: u32;
    /// Upgrade `doc` from schema `from` to `from + 1` (called until current).
    fn migrate(doc: Value, from: u32) -> Value;
    /// Clamp/normalize values after loading or updating.
    fn sanitize(self) -> Self {
        self
    }
    /// Item-by-item recovery when the document does not decode as a whole. Push every raw item
    /// that had to be dropped into `dropped`. `None` means the document is unusable.
    fn salvage(_doc: &Value, _dropped: &mut Vec<Value>) -> Option<Self> {
        None
    }
    /// Parts of the value that live in their own files.
    fn sidecars(&self) -> Vec<Sidecar> {
        vec![]
    }
    /// Merge sidecar files into the freshly loaded value.
    fn with_sidecars(self, _dir: &Path, _report: &mut LoadReport) -> Self {
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    Write(String),
    Serialize(String),
    /// The file could not be read at startup; nothing is written so it cannot be overwritten.
    ReadOnly,
}
impl Coded for StoreError {
    fn code(&self) -> &'static str {
        "error.store.write"
    }
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write(d) | Self::Serialize(d) => write!(f, "error.store.write ({d})"),
            Self::ReadOnly => write!(f, "error.store.write (read-only)"),
        }
    }
}
impl std::error::Error for StoreError {}

/// What happened while loading, for diagnostics (and an optional UI notice).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// File was unreadable; this is where the original went.
    pub recovered_from: Option<PathBuf>,
    pub migrated_from: Option<u32>,
    /// File came from a newer app version (copied to a `.bak`).
    pub newer_version: Option<u32>,
    /// How many items were dropped one by one (see `quarantine_file`).
    pub quarantined: usize,
    pub quarantine_file: Option<PathBuf>,
    /// The file exists but cannot be read (or a safety copy could not be made): the store is
    /// read-only, shows defaults and never writes.
    pub read_only: bool,
}

fn version_of(doc: &Value) -> Option<u32> {
    match doc.get("version") {
        None => Some(0),
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()),
    }
}

/// Decode a document, migrating older versions. `Err` means the shape is unusable.
pub(crate) fn decode<T: Versioned>(
    mut doc: Value,
    report: &mut LoadReport,
    dropped: &mut Vec<Value>,
) -> Result<T, ()> {
    let from = version_of(&doc).ok_or(())?;
    if !doc.is_object() {
        return Err(());
    }
    if from > T::VERSION {
        report.newer_version = Some(from);
    } else if from < T::VERSION {
        report.migrated_from = Some(from);
        for step in from..T::VERSION {
            doc = T::migrate(doc, step);
        }
    }
    match serde_json::from_value::<T>(doc.clone()) {
        Ok(v) => Ok(v.sanitize()),
        Err(_) => T::salvage(&doc, dropped).map(Versioned::sanitize).ok_or(()),
    }
}

fn quarantine(dir: &Path, file: &str, dropped: &[Value], report: &mut LoadReport) {
    report.quarantined = dropped.len();
    eprintln!(
        "OpenWave: {} unreadable item(s) in {file} set aside",
        dropped.len()
    );
    let name = format!("{file}.quarantine-{}.json", persist::stamp());
    let bytes = serde_json::to_vec_pretty(&Value::Array(dropped.to_vec())).unwrap_or_default();
    match persist::write_atomic(dir, &name, &bytes) {
        Ok(()) => report.quarantine_file = Some(dir.join(name)),
        // Without the safety copy the dropped items would be lost on the next save.
        Err(_) => report.read_only = true,
    }
}

pub fn load<T: Versioned>(dir: &Path) -> (T, LoadReport) {
    let mut report = LoadReport::default();
    let path = dir.join(T::FILE);
    let base = match persist::read_json(dir, T::FILE) {
        Read::Missing => T::default().sanitize(),
        Read::Unreadable { error } => {
            eprintln!(
                "OpenWave: {} cannot be read ({error}); keeping it untouched",
                T::FILE
            );
            report.read_only = true;
            return (T::default().sanitize(), report);
        }
        Read::Corrupt { backup } => {
            report.read_only = backup.is_none();
            report.recovered_from = backup;
            T::default().sanitize()
        }
        Read::Value(doc) => {
            let mut dropped = Vec::new();
            match decode::<T>(doc, &mut report, &mut dropped) {
                Ok(value) => {
                    if let Some(n) = report.newer_version {
                        persist::copy_backup(&path, &format!("v{n}.bak"));
                    }
                    if !dropped.is_empty() {
                        quarantine(dir, T::FILE, &dropped, &mut report);
                    }
                    value
                }
                Err(()) => {
                    report.migrated_from = None;
                    report.recovered_from = persist::backup(&path, "corrupt");
                    report.read_only = report.recovered_from.is_none();
                    T::default().sanitize()
                }
            }
        }
    };
    let value = base.with_sidecars(dir, &mut report).sanitize();
    (value, report)
}

pub(crate) fn encode<T: Versioned>(value: &T) -> Result<Vec<u8>, StoreError> {
    let mut doc = serde_json::to_value(value).map_err(|e| StoreError::Serialize(e.to_string()))?;
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("version".into(), T::VERSION.into());
    }
    serde_json::to_vec(&doc).map_err(|e| StoreError::Serialize(e.to_string()))
}
