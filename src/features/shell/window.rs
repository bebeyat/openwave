//! The app window beyond its chrome: closing (to the tray or for real), the tray icon and the
//! compact mini player mode.

use std::cell::Cell;

use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition};
use dioxus::desktop::trayicon::menu::{Menu, MenuItem, PredefinedMenuItem};
use dioxus::desktop::trayicon::{init_tray_icon, Icon};
use dioxus::desktop::{
    use_tray_menu_event_handler, use_window, DesktopContext, WindowCloseBehaviour,
};
use dioxus::prelude::*;

use crate::i18n::t;
use crate::player::controller as ctl;
use crate::store;

/// Size of the mini player window (logical pixels).
pub const MINI_SIZE: (f64, f64) = (380.0, 150.0);

/// The window shows only the mini player.
pub static MINI: GlobalSignal<bool> = Signal::global(|| false);

/// Size, position and maximized state of the full window.
type Placement = (LogicalSize<f64>, PhysicalPosition<i32>, bool);

thread_local! {
    /// Where the full window was, to restore when the mini player closes.
    static BEFORE_MINI: Cell<Option<Placement>> = const { Cell::new(None) };
}

fn hides_on_close() -> bool {
    store::settings().get().close_to_tray
}

/// Work that must happen before the process ends (and only then: a pending update installs here).
fn before_exit() {
    crate::player::session::save_before_exit();
    crate::features::update::install_on_exit();
}

/// Alt+F4, the taskbar or the X button asked to close.
pub fn on_close_requested() {
    if hides_on_close() {
        // Hidden, not gone: keep the saved state fresh in case it is never shown again.
        crate::player::session::save_before_exit();
    } else {
        before_exit();
    }
}

/// The X button: hides to the tray or closes, as set in the settings.
pub fn request_close(window: &DesktopContext) {
    window.set_close_behavior(close_behaviour(hides_on_close()));
    window.close();
}

/// Quit for real, whatever the close setting says.
pub fn quit(window: &DesktopContext) {
    before_exit();
    window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
    window.close();
}

fn close_behaviour(hide: bool) -> WindowCloseBehaviour {
    if hide {
        WindowCloseBehaviour::WindowHides
    } else {
        WindowCloseBehaviour::WindowCloses
    }
}

fn show(window: &DesktopContext) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
}

fn tray_image() -> Option<Icon> {
    const PNG: &[u8] = include_bytes!("../../../assets/branding/openwave-icon-512.png");
    let img = image::load_from_memory(PNG)
        .ok()?
        .resize(32, 32, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Menu entries, kept so their ids can be matched when one is clicked.
#[derive(Clone)]
struct TrayMenu {
    toggle: MenuItem,
    next: MenuItem,
    previous: MenuItem,
    show: MenuItem,
    quit: MenuItem,
}

fn build_menu() -> (Menu, TrayMenu) {
    let items = TrayMenu {
        toggle: MenuItem::new(t("tray.play_pause"), true, None),
        next: MenuItem::new(t("tray.next"), true, None),
        previous: MenuItem::new(t("tray.previous"), true, None),
        show: MenuItem::new(t("tray.show"), true, None),
        quit: MenuItem::new(t("tray.quit"), true, None),
    };
    let menu = Menu::new();
    let added = menu.append_items(&[
        &items.toggle,
        &items.next,
        &items.previous,
        &PredefinedMenuItem::separator(),
        &items.show,
        &items.quit,
    ]);
    if let Err(e) = added {
        eprintln!("tray: {e}");
    }
    (menu, items)
}

/// Mount once (in the shell): the tray icon, its menu and the close behaviour.
#[component]
pub fn TrayHost() -> Element {
    use_tray();
    rsx! {}
}

fn use_tray() {
    let window = use_window();
    let items = use_hook(|| {
        let (menu, items) = build_menu();
        let tray = init_tray_icon(menu, tray_image());
        let _ = tray.set_tooltip(Some("OpenWave"));
        items
    });
    let close_to_tray = *crate::features::settings::boot::CLOSE_TO_TRAY.read();
    let w = window.clone();
    use_effect(use_reactive!(|close_to_tray| {
        w.set_close_behavior(close_behaviour(close_to_tray));
    }));
    use_tray_menu_event_handler(move |event| {
        let id = event.id();
        if id == items.toggle.id() {
            ctl::toggle_pause();
        } else if id == items.next.id() {
            ctl::next();
        } else if id == items.previous.id() {
            ctl::previous();
        } else if id == items.show.id() {
            show(&window);
        } else if id == items.quit.id() {
            quit(&window);
        }
    });
}

/// Switch between the full window and the small always-on-top mini player.
pub fn toggle_mini(window: &DesktopContext) {
    let entering = !*MINI.peek();
    if entering {
        let scale = window.scale_factor();
        let size = window.inner_size().to_logical::<f64>(scale);
        let pos = window.outer_position().unwrap_or_default();
        BEFORE_MINI.set(Some((size, pos, window.is_maximized())));
        window.set_maximized(false);
        window.set_min_inner_size(Some(LogicalSize::new(MINI_SIZE.0, MINI_SIZE.1)));
        window.set_inner_size(LogicalSize::new(MINI_SIZE.0, MINI_SIZE.1));
        window.set_always_on_top(true);
    } else {
        window.set_always_on_top(false);
        window.set_min_inner_size(Some(LogicalSize::new(crate::MIN_SIZE.0, crate::MIN_SIZE.1)));
        if let Some((size, pos, maximized)) = BEFORE_MINI.take() {
            window.set_inner_size(size);
            window.set_outer_position(pos);
            window.set_maximized(maximized);
        }
    }
    *MINI.write() = entering;
}
