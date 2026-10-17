//! Reactive access to the library store for the discovery screens.
//!
//! `store::library()` is not a signal, so every write made by these screens goes through
//! [`update_library`], which bumps [`LIBRARY_REV`]. Components that call [`library`] re-render
//! after such a write. Writes made elsewhere (other screens) should bump the revision the same way.

use std::sync::Arc;

use dioxus::prelude::*;

use crate::i18n::t;
use crate::model::Track;
use crate::store::{self, Library};
use crate::ui::toast_error;

pub static LIBRARY_REV: GlobalSignal<u64> = Signal::global(|| 0);

/// Current library snapshot; subscribes the calling component to [`LIBRARY_REV`].
pub fn library() -> Arc<Library> {
    let _ = *LIBRARY_REV.read();
    store::library().get()
}

/// Apply a pure library change, persist it, tell subscribers, and report a failed write.
pub fn update_library(f: impl FnOnce(&Library) -> Library) {
    if store::library().update(f).is_err() {
        toast_error(t("error.store.write"));
    }
    *LIBRARY_REV.write() += 1;
}

pub fn toggle_like(track: &Track) {
    update_library(|lib| lib.toggle_like(track));
}

/// Remember a search term (most recent first).
pub fn remember_search(term: &str) {
    update_library(|lib| lib.remember_search(term));
}
