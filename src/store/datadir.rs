//! Ownership of the data directory.
//!
//! `OPENWAVE_DATA_DIR` can point anywhere, so "delete all my data" must be sure the folder is
//! really OpenWave's. OpenWave writes a marker file when it creates or adopts a directory; the
//! wipe refuses any directory without it and any folder that is (or contains) the user's home.

use std::path::{Path, PathBuf};

pub const MARKER: &str = ".openwave-data";
pub const LOCK_FILE: &str = ".openwave.lock";
pub const CRASH_LOG: &str = "crash.log";
/// A share link handed over by a second launch (see `features::share::inbox`).
pub const OPEN_LINK: &str = "open-link.txt";

/// Base names of the documents OpenWave keeps in its data directory (each may also exist with a
/// `.tmp`, `.bak`, `.corrupt-*` or `.quarantine-*` suffix).
pub const OWN_FILES: [&str; 10] = [
    "settings.json",
    "library.json",
    "last-queue.json",
    "home_mixes.json",
    "home_recs.json",
    "home_genres.json",
    "home_discover.json",
    "local-files.json",
    OPEN_LINK,
    crate::backend::spotify_auth::FILE,
];

/// Why a directory may not be wiped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No marker: the folder was not created by OpenWave.
    NotOurs,
    /// A drive root, the home folder, a parent of it, or one of the user's personal folders.
    Protected,
}

pub fn has_marker(dir: &Path) -> bool {
    dir.join(MARKER).is_file()
}

fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none())
}

/// Mark `dir` as OpenWave's. Done only when the directory is new, empty, or the default per-user
/// location. Any other non-empty folder is never marked, even if it holds files named like
/// OpenWave's (a custom `OPENWAVE_DATA_DIR` that was not created by OpenWave must not become wipeable).
/// Returns whether the marker is in place afterwards.
pub fn ensure_marker(dir: &Path, is_default_location: bool) -> bool {
    if has_marker(dir) {
        return true;
    }
    if is_protected(dir) {
        return false;
    }
    let adopt = !dir.exists() || is_default_location || is_empty_dir(dir);
    adopt
        && std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(dir.join(MARKER), b"OpenWave data folder\n"))
            .is_ok()
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Folders that must never be wiped: drive roots, the home folder and every parent of it, and
/// the user's well-known personal folders.
pub fn is_protected(dir: &Path) -> bool {
    let dir = canonical(dir);
    if dir.parent().is_none() {
        return true;
    }
    let mut personal: Vec<PathBuf> = Vec::new();
    if let Some(base) = directories::BaseDirs::new() {
        personal.push(base.home_dir().to_path_buf());
        personal.extend(
            [base.data_dir(), base.data_local_dir(), base.config_dir()].map(Path::to_path_buf),
        );
    }
    if let Some(user) = directories::UserDirs::new() {
        personal.extend(
            [
                user.desktop_dir(),
                user.document_dir(),
                user.download_dir(),
                user.audio_dir(),
                user.video_dir(),
                user.picture_dir(),
            ]
            .into_iter()
            .flatten()
            .map(Path::to_path_buf),
        );
    }
    let home = directories::BaseDirs::new().map(|b| canonical(b.home_dir()));
    // `dir` is the home folder or one of its parents (C:\Users).
    if home.is_some_and(|h| h.starts_with(&dir)) {
        return true;
    }
    personal.iter().any(|p| canonical(p) == dir)
}

/// `Ok` when `dir` may be wiped.
pub fn check_wipe_target(dir: &Path) -> Result<(), Refusal> {
    if is_protected(dir) {
        Err(Refusal::Protected)
    } else if !has_marker(dir) {
        Err(Refusal::NotOurs)
    } else {
        Ok(())
    }
}

/// Write the marker (tests only; the app writes it through `ensure_marker`).
#[cfg(test)]
pub fn write_marker(dir: &Path) -> std::io::Result<()> {
    std::fs::write(dir.join(MARKER), b"OpenWave data folder\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ow-datadir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn empty_or_new_folders_are_adopted() {
        let d = temp();
        assert!(ensure_marker(&d, false));
        assert!(has_marker(&d));
        let fresh = d.join("new");
        assert!(ensure_marker(&fresh, false));
        assert!(has_marker(&fresh));
    }

    #[test]
    fn foreign_folders_are_not_adopted_even_with_openwave_named_files() {
        let foreign = temp();
        std::fs::write(foreign.join("holiday.jpg"), b"x").unwrap();
        assert!(!ensure_marker(&foreign, false));
        assert!(!has_marker(&foreign));
        assert_eq!(check_wipe_target(&foreign), Err(Refusal::NotOurs));

        let lookalike = temp();
        std::fs::write(lookalike.join("library.json"), b"{}").unwrap();
        std::fs::write(lookalike.join("holiday.jpg"), b"x").unwrap();
        assert!(!ensure_marker(&lookalike, false));
        assert_eq!(check_wipe_target(&lookalike), Err(Refusal::NotOurs));

        let ours = temp();
        assert!(ensure_marker(&ours, false));
        std::fs::write(ours.join("library.json"), b"{}").unwrap();
        assert!(ensure_marker(&ours, false));
        assert_eq!(check_wipe_target(&ours), Ok(()));
    }

    #[test]
    fn roots_home_and_personal_folders_are_protected() {
        let root = std::env::current_dir()
            .unwrap()
            .ancestors()
            .last()
            .unwrap()
            .to_path_buf();
        assert!(is_protected(&root));
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert!(is_protected(&home));
        assert!(is_protected(home.parent().unwrap_or(&home)));
        if let Some(docs) =
            directories::UserDirs::new().and_then(|u| u.document_dir().map(Path::to_path_buf))
        {
            assert!(is_protected(&docs));
        }
        assert!(!is_protected(&temp()));
    }

    #[test]
    fn marker_is_never_written_into_a_protected_folder() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert!(!ensure_marker(&home, true));
        assert!(!home.join(MARKER).exists());
    }
}
