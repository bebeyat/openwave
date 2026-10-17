//! Root component: global CSS, theme sync, router and the toast host.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::ui::theme::use_theme_sync;
use crate::ui::ToastHost;

/// Compiled by `npm run css` in `tools/`; embedded so there is no file or network lookup.
pub const MAIN_CSS: &str = include_str!("../assets/main.css");
/// Inter Variable as data URIs (see `tools/extract-assets.mjs`).
pub const FONTS_CSS: &str = include_str!("../assets/fonts.css");

/// Content-Security-Policy of the web view. Everything the page loads is embedded (styles, fonts as
/// data URIs), so the only network targets are cover pictures (image hosts), the loopback audio
/// proxy and the loopback WebSocket Dioxus uses to drive the page. Inline scripts must stay
/// allowed: besides our bridge, Dioxus injects two inline bootstrap scripts whose text contains the
/// per-launch WebSocket address and key, so they cannot be pinned by a hash or a nonce (checked in
/// the real app: hash-only blocks them and the window stays empty). No `eval`, frames or plugins.
///
/// Sound effects (pitch shifting) run in an AudioWorklet whose module is built from a `Blob` URL by
/// the bundled signalsmith-stretch library, and the DSP is WebAssembly. Worklet modules are governed
/// by `script-src`, so it additionally allows `blob:` (script text the page itself created; there is
/// no remote code) and `'wasm-unsafe-eval'` (compile WebAssembly only, `eval` stays blocked).
pub const CSP: &str = concat!(
    "default-src 'self'; ",
    "img-src 'self' data: http://127.0.0.1:* https://*.ytimg.com https://*.ggpht.com https://*.googleusercontent.com; ",
    "media-src http://127.0.0.1:*; ",
    "connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; ",
    "style-src 'self' 'unsafe-inline'; ",
    "script-src 'self' 'unsafe-inline' blob: 'wasm-unsafe-eval'; ",
    "font-src 'self' data:; ",
    "object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'"
);

/// Origin of the app's own web view (what WebView2/wry report as `location.origin`). The media proxy
/// answers CORS for exactly this origin, and it is the one origin treated as a secure context.
pub const WEBVIEW_ORIGIN: &str = "http://dioxus.index.html";

/// Extra WebView2 arguments. Giving any replaces wry's defaults, so those are repeated here
/// (`msWebOOUI` etc.: no mini menu / smart screen; autoplay: playback without a prior click).
/// `--unsafely-treat-insecure-origin-as-secure` makes the app's own page a secure context, which
/// Chromium requires for `AudioWorklet` (the pitch shifter of the sound effects); the page only ever
/// loads embedded content, so this widens nothing for the outside world.
pub fn browser_args() -> String {
    format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --unsafely-treat-insecure-origin-as-secure={WEBVIEW_ORIGIN}"
    )
}

/// HTML injected into `<head>` before first paint: the CSP, styles, font and the JS bridge.
pub fn head_html() -> String {
    format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\"><style>{FONTS_CSS}</style><style>{MAIN_CSS}</style><script>{}</script><script>{}</script>",
        crate::ui::js::BRIDGE_JS,
        crate::theme::THEME_JS
    )
}

/// Debug builds only: `OPENWAVE_START=/library/playlists`, `OPENWAVE_THEME=dark|light`,
/// `OPENWAVE_ACCENT=blue`, `OPENWAVE_LOW_POWER=1` make screenshots reproducible.
#[cfg(debug_assertions)]
fn apply_debug_env() {
    use crate::ui::{Accent, Theme, ACCENT, LOW_POWER, THEME};
    if let Ok(v) = std::env::var("OPENWAVE_THEME") {
        *THEME.write() = Theme::parse(&v);
    }
    if let Ok(v) = std::env::var("OPENWAVE_ACCENT") {
        *ACCENT.write() = Accent::parse(&v);
    }
    if std::env::var("OPENWAVE_LOW_POWER").is_ok() {
        *LOW_POWER.write() = true;
    }
}

/// Debug builds only: start the router at `OPENWAVE_START`.
#[cfg(debug_assertions)]
fn provide_debug_start() {
    use dioxus::history::{History, MemoryHistory};
    use std::rc::Rc;
    if let Ok(path) = std::env::var("OPENWAVE_START") {
        use_context_provider(|| Rc::new(MemoryHistory::with_initial_path(path)) as Rc<dyn History>);
    }
}

/// Debug builds only: `OPENWAVE_SCROLL=700` scrolls the content area after start.
#[cfg(debug_assertions)]
fn debug_scroll() {
    use_future(|| async {
        if let Ok(px) = std::env::var("OPENWAVE_SCROLL") {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            document::eval(&format!(
                "document.getElementById('ow-main').scrollTop={px}"
            ));
        }
    });
}

#[component]
pub fn App() -> Element {
    #[cfg(debug_assertions)]
    {
        use_hook(apply_debug_env);
        provide_debug_start();
        debug_scroll();
    }
    use_theme_sync();
    crate::theme::use_ambient_theme();
    crate::features::settings::boot::use_custom_style();
    crate::features::discord::use_discord_presence();
    use_hook(crate::features::downloads::refresh);
    rsx! {
        crate::theme::Backdrop {}
        Router::<Route> {}
        ToastHost {}
    }
}

#[cfg(test)]
mod csp_tests {
    use super::*;

    #[test]
    fn browser_args_keep_wrys_defaults_and_name_only_our_origin() {
        let args = browser_args();
        assert!(
            args.contains("msSmartScreenProtection") && args.contains("no-user-gesture-required")
        );
        assert!(args.ends_with("=http://dioxus.index.html"));
        assert_eq!(
            args.matches("--unsafely-treat-insecure-origin-as-secure=")
                .count(),
            1
        );
    }

    #[test]
    fn csp_comes_first_and_stays_tight() {
        let head = head_html();
        let csp = head.find("Content-Security-Policy").expect("CSP meta");
        assert!(csp < head.find("<style>").unwrap() && csp < head.find("<script>").unwrap());
        for needed in [
            "default-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
            "frame-src 'none'",
            "media-src http://127.0.0.1:*",
        ] {
            assert!(CSP.contains(needed), "{needed}");
        }
        assert!(!CSP.contains("'unsafe-eval'"));
        // Only the three image hosts and loopback may be named; no catch-all sources.
        for token in CSP.split([' ', ';']) {
            assert!(
                !["*", "http:", "https:", "ws:", "wss:"].contains(&token),
                "catch-all source {token}"
            );
        }
        // `blob:` is allowed for the effects worklet in script-src only.
        let script = CSP
            .split("; ")
            .find(|d| d.starts_with("script-src"))
            .unwrap();
        assert!(script.contains(" blob:") && script.contains("'wasm-unsafe-eval'"));
        assert_eq!(CSP.matches("blob:").count(), 1, "blob: only in script-src");
        assert!(
            !CSP.contains('"'),
            "must not break out of the content attribute"
        );
    }
}
