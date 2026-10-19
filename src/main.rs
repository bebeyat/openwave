#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod features;
mod i18n;
mod model;
mod player;
mod routes;
mod startup;
mod store;
mod theme;
mod ui;
mod util;

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{Config, WindowBuilder};

const MIN_SIZE: (f64, f64) = (960.0, 640.0);
const START_SIZE: (f64, f64) = (1200.0, 800.0);

/// Debug builds only: `OPENWAVE_SIZE=960x640` for reproducible screenshots.
fn start_size() -> (f64, f64) {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("OPENWAVE_SIZE") {
        if let Some((w, h)) = v.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                return (w, h);
            }
        }
    }
    START_SIZE
}

/// Debug builds only: Dioxus restores the last window position from `window-state.json` in the
/// temp folder. A window closed while minimized is saved at (-32000, -32000), so the next start
/// opens off screen (taskbar entry, nothing visible). Such a file is dropped before launch.
#[cfg(debug_assertions)]
fn drop_offscreen_window_state() {
    const OFFSCREEN: i64 = -10_000;
    let path = std::env::temp_dir().join("window-state.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(state) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let coord = |k: &str| state[k].as_i64().unwrap_or(0);
    if coord("x") <= OFFSCREEN || coord("y") <= OFFSCREEN {
        // Best effort: if it cannot be removed the window is only misplaced, as before.
        let _ = std::fs::remove_file(&path);
    }
}

/// The app icon for the window and the taskbar (the exe icon only covers Explorer, and only in
/// builds that embed it).
fn window_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    const PNG: &[u8] = include_bytes!("../assets/branding/openwave-icon-512.png");
    let img =
        image::load_from_memory(PNG)
            .ok()?
            .resize(256, 256, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    dioxus::desktop::tao::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
}

fn window_config() -> Config {
    #[cfg(debug_assertions)]
    drop_offscreen_window_state();
    let (width, height) = start_size();
    let window = WindowBuilder::new()
        .with_title("OpenWave")
        .with_decorations(false)
        // Transparent so the page can round the window corners itself (Windows 10 has no
        // rounded corners for frameless windows); see `html { clip-path }` in the stylesheet.
        .with_transparent(true)
        .with_inner_size(LogicalSize::new(width, height))
        .with_min_inner_size(LogicalSize::new(MIN_SIZE.0, MIN_SIZE.1));
    // The "undecorated shadow" makes Windows 10 draw a square 1px frame around the window, which
    // shows outside the rounded corners. Without it tao still handles resizing on every edge.
    #[cfg(windows)]
    let window = {
        use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
        window.with_undecorated_shadow(false)
    };
    let window = window.with_window_icon(window_icon());
    let config = Config::new()
        .with_window(window)
        .with_menu(None)
        .with_background_color((0, 0, 0, 0))
        .with_custom_head(app::head_html())
        .with_windows_browser_args(app::browser_args());
    with_debug_cdp(config)
}

/// Debug builds only: `OPENWAVE_CDP=9333` opens the WebView2 DevTools port so
/// a local DevTools client can drive real input and capture screenshots.
fn with_debug_cdp(config: Config) -> Config {
    #[cfg(debug_assertions)]
    if let Ok(port) = std::env::var("OPENWAVE_CDP") {
        let args = format!("{} --remote-debugging-port={port}", app::browser_args());
        return config.with_windows_browser_args(args);
    }
    config
}

fn main() {
    let dir = backend::data_dir();
    startup::install_panic_hook(dir.clone());
    if !startup::webview2_installed() {
        startup::report_missing_webview2();
        std::process::exit(1);
    }
    // Mark the folder as ours before anything else is created in it (the wipe checks this).
    store::datadir::ensure_marker(&dir, std::env::var_os("OPENWAVE_DATA_DIR").is_none());
    // Two instances must never write the same files. Held until the process exits.
    let _instance = match startup::try_lock(&dir) {
        startup::Instance::First(guard) => guard,
        startup::Instance::Running => {
            startup::focus_running_window();
            return;
        }
    };
    // Runtime, media proxy and catalog client must exist before any screen calls the API.
    if let Err(e) = backend::init() {
        startup::report_init_failure(&dir, &e);
        std::process::exit(2);
    }
    dioxus::LaunchBuilder::desktop()
        .with_cfg(window_config())
        .launch(app::App);
    // Never reached: the window loop ends the process. Saving happens on close (see `WindowControls`).
}
