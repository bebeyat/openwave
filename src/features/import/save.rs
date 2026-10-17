//! Saving an import into the library. Nothing is written before the user presses
//! "Kitaplığıma ekle"; this is the single place that writes.

use std::path::PathBuf;

use crate::backend::api;
use crate::backend::error::{ApiError, Coded};
use crate::i18n::t;
use crate::model::Track;
use crate::store::library::{new_id, now_ms};
use crate::store::{self, Library, StoreError};

const NAME_MAX_CHARS: usize = 100;

/// The playlist name to save: trimmed, capped, with a friendly fallback for blank names.
pub fn final_name(typed: &str, fallback: &str) -> String {
    let name: String = typed.trim().chars().take(NAME_MAX_CHARS).collect();
    if name.is_empty() {
        fallback.to_owned()
    } else {
        name
    }
}

/// `lib` plus a new playlist holding `tracks` (duplicates are skipped by the library).
pub fn with_playlist(lib: &Library, id: &str, name: &str, tracks: &[Track], now: u64) -> Library {
    lib.create_playlist(id, name, now)
        .add_to_playlist(id, tracks, now)
}

/// Create the playlist and persist it. Returns the new playlist id.
pub fn save_playlist(name: &str, tracks: &[Track]) -> Result<String, StoreError> {
    let id = new_id().to_string();
    let name = final_name(name, &t("import.default_name"));
    let now = now_ms();
    store::library().update(|lib| with_playlist(lib, &id, &name, tracks, now))?;
    Ok(id)
}

/// Remember local files in the library ("Bilgisayarım").
pub fn save_local(tracks: &[Track]) -> Result<(), StoreError> {
    store::library().update(|lib| lib.add_local_files(tracks))?;
    Ok(())
}

/// Register dropped or picked audio paths and remember them. Returns how many songs were added.
pub async fn add_paths(paths: Vec<PathBuf>) -> Result<usize, String> {
    let tracks = api::add_local_files(paths)
        .await
        .map_err(|e: ApiError| e.user_message())?;
    save_local(&tracks).map_err(|e| e.user_message())?;
    Ok(tracks.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.into(),
            title: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn blank_and_long_names_are_normalised() {
        assert_eq!(final_name("  Yolculuk  ", "x"), "Yolculuk");
        assert_eq!(final_name("   ", "Liste"), "Liste");
        assert_eq!(final_name(&"a".repeat(300), "x").chars().count(), 100);
    }

    #[test]
    fn playlist_is_added_without_touching_the_original_library() {
        let before = Library::default();
        let after = with_playlist(
            &before,
            "p1",
            "Mix",
            &[track("a"), track("b"), track("a")],
            7,
        );
        assert!(before.playlist("p1").is_none());
        let list = after.playlist("p1").expect("created");
        assert_eq!(list.name.as_str(), "Mix");
        assert_eq!(list.tracks.len(), 2, "duplicates collapse");
    }
}
