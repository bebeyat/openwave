//! Checks that run before the window exists: WebView2 runtime, single instance, crash log, and a
//! native message box for fatal errors (a release build has no console, so `eprintln!` is
//! invisible). Messages here are plain Turkish/English chosen from the OS language because the
//! app's own translations are not loaded yet.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::store::datadir::{CRASH_LOG, LOCK_FILE};

pub const WEBVIEW2_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";
const CRASH_LOG_MAX: u64 = 256 * 1024;

fn turkish() -> bool {
    sys_locale::get_locale().is_some_and(|l| l.to_ascii_lowercase().starts_with("tr"))
}

fn pick(tr: &'static str, en: &'static str) -> &'static str {
    if turkish() {
        tr
    } else {
        en
    }
}

/// Show a native error box. Never panics, works without a window or an async runtime.
pub fn show_error(title: &str, text: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

// ---- WebView2 ----------------------------------------------------------------------------

/// True when `root` holds an installed Evergreen runtime: `<root>\Microsoft\EdgeWebView\
/// Application\<version>\msedgewebview2.exe`.
pub fn runtime_in(root: &Path) -> bool {
    let apps = root
        .join("Microsoft")
        .join("EdgeWebView")
        .join("Application");
    std::fs::read_dir(apps).is_ok_and(|versions| {
        versions
            .flatten()
            .any(|v| v.path().join("msedgewebview2.exe").is_file())
    })
}

/// Whether the WebView2 runtime is available. A user-provided runtime folder
/// (`WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`) counts. Other platforms are not checked.
pub fn webview2_installed() -> bool {
    if !cfg!(windows) || std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER").is_some() {
        return true;
    }
    ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|root| runtime_in(Path::new(&root)))
}

/// Tell the user the runtime is missing and offer the official download page.
pub fn report_missing_webview2() {
    let text = pick(
        "OpenWave çalışmak için Microsoft WebView2 çalışma zamanına ihtiyaç duyar, ama bu bilgisayarda bulunamadı.\n\nİndirme sayfası:\nhttps://developer.microsoft.com/microsoft-edge/webview2/\n\nSayfayı şimdi açmak ister misiniz?",
        "OpenWave needs the Microsoft WebView2 runtime, but it was not found on this computer.\n\nDownload page:\nhttps://developer.microsoft.com/microsoft-edge/webview2/\n\nOpen the page now?",
    );
    let answer = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(pick("OpenWave başlatılamadı", "OpenWave could not start"))
        .set_description(text)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();
    if answer == rfd::MessageDialogResult::Yes {
        let _ = open::that(WEBVIEW2_URL);
    }
}

/// Tell the user the engine failed to start (and log the technical reason).
pub fn report_init_failure(dir: &Path, error: &dyn std::fmt::Display) {
    write_crash_log(dir, &format!("backend::init failed: {error}"));
    let text = pick(
        "OpenWave'in motoru başlatılamadı.\n\nVeri klasörü yazılabilir mi ve başka bir program tarafından kilitli mi, kontrol edin. Ayrıntılar crash.log dosyasında:\n",
        "OpenWave's engine could not be started.\n\nCheck that the data folder is writable and not locked by another program. Details are in crash.log:\n",
    );
    show_error(
        pick("OpenWave başlatılamadı", "OpenWave could not start"),
        &format!("{text}{}", dir.join(CRASH_LOG).display()),
    );
}

// ---- crash log ---------------------------------------------------------------------------

/// Append an entry to `crash.log` in the data directory. Stays on this computer.
pub fn write_crash_log(dir: &Path, entry: &str) {
    use std::io::Write;
    let path = dir.join(CRASH_LOG);
    let _ = std::fs::create_dir_all(dir);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > CRASH_LOG_MAX) {
        let _ = std::fs::remove_file(&path);
    }
    let when = crate::store::library::now_ms();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[{when}] {entry}\n");
    }
}

/// Write panics to `crash.log` (nothing is uploaded), then run the default hook.
pub fn install_panic_hook(dir: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        write_crash_log(&dir, &format!("panic: {info}\n{backtrace}"));
        default(info);
    }));
}

// ---- single instance ---------------------------------------------------------------------

pub enum Instance {
    /// This process owns the data directory; keep the guard alive until exit.
    First(Option<File>),
    /// Another OpenWave is running on the same data directory.
    Running,
}

/// Open `<dir>/.openwave.lock` exclusively (Windows share mode 0, an advisory lock elsewhere). The OS releases it when the
/// process ends, even after a crash, so a stale file never blocks the next start.
pub fn try_lock(dir: &Path) -> Instance {
    let _ = std::fs::create_dir_all(dir);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    match options.open(dir.join(LOCK_FILE)) {
        // Unix has no share mode: an advisory lock does the same job (released on exit too).
        #[cfg(not(windows))]
        Ok(file) if matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock)) => {
            Instance::Running
        }
        Ok(file) => Instance::First(Some(file)),
        Err(e) if is_in_use(&e) => Instance::Running,
        // Cannot create the lock for some other reason: do not block the app over it.
        Err(_) => Instance::First(None),
    }
}

fn is_in_use(e: &std::io::Error) -> bool {
    // ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION
    cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33))
}

/// Bring the already running window to the front (best effort).
pub fn focus_running_window() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn FindWindowW(class: *const u16, title: *const u16) -> isize;
            fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
            fn SetForegroundWindow(hwnd: isize) -> i32;
        }
        const SW_RESTORE: i32 = 9;
        let title: Vec<u16> = "OpenWave\0".encode_utf16().collect();
        // SAFETY: plain Win32 calls with a NUL-terminated UTF-16 string that outlives them.
        unsafe {
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if hwnd != 0 {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ow-startup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).expect("tmp");
        d
    }

    #[test]
    fn runtime_is_found_only_with_the_exe_in_a_version_folder() {
        let root = temp();
        assert!(!runtime_in(&root));
        let app = root.join("Microsoft/EdgeWebView/Application");
        std::fs::create_dir_all(app.join("130.0.1")).expect("dir");
        assert!(
            !runtime_in(&root),
            "an empty version folder is not a runtime"
        );
        std::fs::write(app.join("130.0.1/msedgewebview2.exe"), b"x").expect("exe");
        assert!(runtime_in(&root));
    }

    #[test]
    fn second_instance_is_detected_and_a_stale_lock_file_does_not_block() {
        let d = temp();
        let first = try_lock(&d);
        assert!(matches!(first, Instance::First(Some(_))));
        assert!(matches!(try_lock(&d), Instance::Running));
        drop(first);
        assert!(
            matches!(try_lock(&d), Instance::First(_)),
            "the lock file stays on disk but is free once the owner is gone"
        );
    }

    #[test]
    fn crash_log_appends_and_is_trimmed() {
        let d = temp();
        write_crash_log(&d, "first");
        write_crash_log(&d, "second");
        let text = std::fs::read_to_string(d.join(CRASH_LOG)).expect("log");
        assert!(text.contains("first") && text.contains("second"));
        std::fs::write(d.join(CRASH_LOG), vec![b'x'; (CRASH_LOG_MAX + 1) as usize]).expect("big");
        write_crash_log(&d, "fresh");
        let text = std::fs::read_to_string(d.join(CRASH_LOG)).expect("log");
        assert!(text.contains("fresh") && text.len() < 1000);
    }
}
