//! "Tüm verilerimi sil": removes only the files OpenWave itself creates inside its data directory,
//! matched by name pattern. Anything else in that folder (for example when `OPENWAVE_DATA_DIR`
//! points at a shared directory) is never touched, and the wipe refuses folders that do not carry
//! OpenWave's marker file or that are drive roots / the user's home or personal folders.

use std::path::Path;

use crate::store::datadir::{self, Refusal, CRASH_LOG, OWN_FILES};

/// Outcome of a wipe. Files that are in use are reported, not fatal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WipeReport {
    pub deleted: usize,
    /// Names of files that could not be deleted (locked by another process).
    pub locked: Vec<String>,
}

/// `settings.json`, `library.json.tmp`, `library.json.corrupt-123`, `library.json.v2.bak`,
/// `library.json.quarantine-9.json`, `local-files.<uuid>.tmp` (older versions), `crash.log` ...
pub fn is_own_file(name: &str) -> bool {
    if name == CRASH_LOG || (name.starts_with("local-files.") && name.ends_with(".tmp")) {
        return true;
    }
    OWN_FILES.iter().any(|base| {
        name == *base
            || name
                .strip_prefix(*base)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|rest| {
                    rest.ends_with(".tmp")
                        || rest.starts_with("corrupt-")
                        || rest.starts_with("quarantine-")
                        || is_version_backup(rest)
                })
    })
}

/// `v12.bak`
fn is_version_backup(s: &str) -> bool {
    s.strip_prefix('v')
        .and_then(|r| r.strip_suffix(".bak"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `<track id>.audio | .json | .part` (11-char video ids and `sc-<digits>` SoundCloud ids)
pub fn is_cache_file(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    crate::backend::media::video_key(stem) && matches!(ext, "audio" | "json" | "part")
}

/// `<16 hex>.jpg`: a cover thumbnail of a local file.
pub fn is_cover_file(name: &str) -> bool {
    name.strip_suffix(".jpg")
        .is_some_and(crate::backend::local_tags::valid_cover_hash)
}

/// `yt-dlp.exe`, `yt-dlp.exe.part`, `new-yt-dlp.exe`, ...
pub fn is_helper_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("yt-dlp") || lower.starts_with("new-yt-dlp")
}

fn delete_matching(dir: &Path, is_ours: impl Fn(&str) -> bool, report: &mut WipeReport) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !is_ours(&name) || !path.is_file() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.deleted += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => report.locked.push(name),
        }
    }
}

/// Delete OpenWave's own data in `dir`. Refuses folders that are not clearly OpenWave's.
pub fn wipe_own_data(dir: &Path) -> Result<WipeReport, Refusal> {
    datadir::check_wipe_target(dir)?;
    let mut report = WipeReport::default();
    delete_matching(dir, is_own_file, &mut report);
    delete_matching(&dir.join("cache"), is_cache_file, &mut report);
    // Offline downloads use the same `<id>.audio|json|part` names as the cache.
    delete_matching(&dir.join("downloads"), is_cache_file, &mut report);
    delete_matching(&dir.join("covers"), is_cover_file, &mut report);
    delete_matching(&dir.join("bin"), is_helper_file, &mut report);
    delete_matching(
        &dir.join(crate::backend::lyrics_cache::DIR),
        crate::backend::lyrics_cache::is_cache_file,
        &mut report,
    );
    delete_matching(
        &dir.join(crate::backend::update::DIR),
        |n| n.starts_with("OpenWave-") && (n.ends_with("-setup.exe") || n.ends_with("-setup.part")),
        &mut report,
    );
    // Only folders that are now empty; anything else in them is not ours.
    for sub in [
        "cache",
        "downloads",
        "covers",
        "bin",
        crate::backend::lyrics_cache::DIR,
        crate::backend::update::DIR,
    ] {
        let _ = std::fs::remove_dir(dir.join(sub));
    }
    Ok(report)
}

/// Stop everything that could write the files again: downloads, playback, then the background
/// writers. The in-memory library and settings are reset to defaults.
pub fn quiesce() {
    crate::features::downloads::cancel_all();
    crate::player::controller::pause();
    crate::player::controller::clear_queue();
    crate::store::halt_all();
}

/// The word the user types to confirm; it names the app, not an action, so it is the same in
/// every language and cannot be typed by accident.
pub const CONFIRM_WORD: &str = "OpenWave";

pub fn confirmation_matches(typed: &str) -> bool {
    typed.trim().eq_ignore_ascii_case(CONFIRM_WORD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ow-wipe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).expect("tmp");
        d
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().expect("parent")).expect("dir");
        std::fs::write(p, b"x").expect("write");
    }

    #[test]
    fn removes_only_known_files_and_keeps_foreign_content() {
        let d = temp();
        datadir::write_marker(&d).expect("marker");
        for f in [
            "settings.json",
            "library.json",
            "last-queue.json",
            "home_mixes.json",
            "local-files.json",
            "library.json.corrupt-123",
            "library.json.v2.bak",
            "library.json.quarantine-5.json",
            "library.json.1234-7.tmp",
            "home_mixes.json.55-1.tmp",
            "local-files.0b6c3a1e-9f2d-4f5e-8a7b-1c2d3e4f5a6b.tmp",
            "crash.log",
            "cache/dQw4w9WgXcQ.audio",
            "cache/dQw4w9WgXcQ.json",
            "cache/abcdefghijk.part",
            "bin/yt-dlp.exe",
            "bin/new-yt-dlp.exe",
        ] {
            touch(&d.join(f));
        }
        // Unrelated things that must survive, including foreign files inside bin and cache.
        touch(&d.join("holiday.jpg"));
        touch(&d.join("library.json.mine.txt"));
        touch(&d.join("local-files.txt"));
        touch(&d.join("Documents/thesis.docx"));
        touch(&d.join("bin/my-tool.exe"));
        touch(&d.join("cache/notes.txt"));

        let r = wipe_own_data(&d).expect("wipe");
        assert_eq!(r.deleted, 17);
        assert!(r.locked.is_empty());
        for gone in [
            "settings.json",
            "library.json",
            "last-queue.json",
            "home_mixes.json",
            "crash.log",
        ] {
            assert!(!d.join(gone).exists(), "{gone}");
        }
        assert!(!d.join("cache/dQw4w9WgXcQ.audio").exists() && !d.join("bin/yt-dlp.exe").exists());
        for kept in [
            "holiday.jpg",
            "library.json.mine.txt",
            "local-files.txt",
            "Documents/thesis.docx",
            "bin/my-tool.exe",
            "cache/notes.txt",
        ] {
            assert!(d.join(kept).exists(), "{kept} must survive");
        }
        assert_eq!(wipe_own_data(&d).expect("again").deleted, 0, "idempotent");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn empty_cache_and_bin_folders_are_removed() {
        let d = temp();
        datadir::write_marker(&d).expect("marker");
        touch(&d.join("cache/dQw4w9WgXcQ.audio"));
        touch(&d.join("bin/yt-dlp.exe"));
        wipe_own_data(&d).expect("wipe");
        assert!(!d.join("cache").exists() && !d.join("bin").exists());
        assert!(
            d.join(datadir::MARKER).exists(),
            "folder stays recognisably ours"
        );
    }

    #[test]
    fn refuses_a_folder_without_the_marker() {
        let d = temp();
        touch(&d.join("bin/compiler.exe"));
        touch(&d.join("cache/index.bin"));
        touch(&d.join("library.json"));
        assert_eq!(wipe_own_data(&d), Err(Refusal::NotOurs));
        assert!(d.join("bin/compiler.exe").exists() && d.join("library.json").exists());
    }

    #[test]
    fn refuses_home_and_roots() {
        let home = directories::BaseDirs::new()
            .expect("home")
            .home_dir()
            .to_path_buf();
        assert_eq!(wipe_own_data(&home), Err(Refusal::Protected));
        let root = std::env::current_dir()
            .expect("cwd")
            .ancestors()
            .last()
            .expect("root")
            .to_path_buf();
        assert_eq!(wipe_own_data(&root), Err(Refusal::Protected));
    }

    #[cfg(windows)]
    #[test]
    fn locked_files_are_reported_and_do_not_abort_the_wipe() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = temp();
        datadir::write_marker(&d).expect("marker");
        touch(&d.join("cache/dQw4w9WgXcQ.audio"));
        touch(&d.join("library.json"));
        touch(&d.join("settings.json"));
        let _open = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(d.join("cache/dQw4w9WgXcQ.audio"))
            .expect("lock");
        let r = wipe_own_data(&d).expect("wipe");
        assert_eq!(r.deleted, 2);
        assert_eq!(r.locked, ["dQw4w9WgXcQ.audio"]);
        assert!(!d.join("library.json").exists() && d.join("cache").is_dir());
    }

    #[test]
    fn confirmation_needs_the_app_name() {
        assert!(confirmation_matches("openwave"));
        assert!(confirmation_matches("  OpenWave "));
        assert!(!confirmation_matches(""));
        assert!(!confirmation_matches("sil"));
    }

    #[test]
    fn only_cover_thumbnails_match() {
        assert!(is_cover_file("0123456789abcdef.jpg"));
        for bad in [
            "0123456789abcdef.png",
            "photo.jpg",
            "0123456789ABCDEF.jpg",
            "../x.jpg",
        ] {
            assert!(!is_cover_file(bad), "{bad}");
        }
    }
}
