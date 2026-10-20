//! JSON persistence in the data directory: versioned documents, migrations, atomic writes.
//!
//! Use the global handles: `store::settings().get()` for a cheap snapshot and
//! `store::library().update(|lib| lib.toggle_like(&track))` to change and persist.
// The UI consumes this API incrementally; unused items are expected until it does.

pub mod backup;
pub mod customize;
pub mod datadir;
mod handle;
pub mod library;
mod library_io;
pub mod persist;
pub mod settings;
pub mod versioned;

use std::path::PathBuf;
use std::sync::OnceLock;

pub use library::Library;
pub use settings::{
    AudioQuality, DoubleClickAction, LanguagePref, ReverbPreset, Settings, SettingsChange,
    ThemePref, ACCENT_IDS,
};
pub use versioned::{Store, StoreError};

static SETTINGS: OnceLock<Store<Settings>> = OnceLock::new();
static LIBRARY: OnceLock<Store<Library>> = OnceLock::new();

/// Global settings store (opened from the data directory on first use).
pub fn settings() -> &'static Store<Settings> {
    SETTINGS.get_or_init(|| Store::open(crate::backend::data_dir()))
}

/// Global library store (opened from the data directory on first use).
pub fn library() -> &'static Store<Library> {
    LIBRARY.get_or_init(|| Store::open(crate::backend::data_dir()))
}

/// Whether the stores are usable; a calm banner shows `read_only` (see `features::store_health`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Health {
    /// A data file exists but cannot be read right now; changes are not saved.
    pub read_only: bool,
    /// The library file was damaged and started empty; the original was kept here.
    pub recovered: Option<PathBuf>,
}

/// Current health. Cheap; also re-tries loading stores that were unreadable, so the warning
/// disappears by itself once the antivirus or sync client lets go of the file.
pub fn health() -> Health {
    let mut read_only = false;
    let mut recovered = None;
    if let Some(s) = SETTINGS.get() {
        read_only |= !s.try_recover();
    }
    if let Some(l) = LIBRARY.get() {
        read_only |= !l.try_recover();
        recovered = l.report().recovered_from;
    }
    Health {
        read_only,
        recovered,
    }
}

/// Write everything pending now (window close, before a restart).
pub fn flush_all() {
    if let Some(s) = SETTINGS.get() {
        let _ = s.flush();
    }
    if let Some(l) = LIBRARY.get() {
        let _ = l.flush();
    }
}

/// Stop writing for good and reset the in-memory data to defaults ("delete all my data").
pub fn halt_all() {
    // Opening is harmless here: a halted store never writes.
    settings().halt();
    library().halt();
}
