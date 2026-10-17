//! Ambient theme engine: the window takes the colors of the current track's cover.
//!
//! `use_ambient_theme()` (called once in `App`) watches the current track's cover URL (read-only
//! view of the player queue) and the low-power flag. For every new cover it asks
//! `backend::palette` for a [`Palette`] (disk-cached) and runs `window.__owTheme.apply(..)`;
//! with nothing playing, or when the palette cannot be built (offline, bad image), it runs
//! `.clear()` and the default accent-preset look returns. `Backdrop` is the DOM the script drives.

pub mod script;

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use crate::backend::palette::{allowed_image_url, palette_for, small_url, Palette};
use crate::player::controller as ctl;
use crate::ui::LOW_POWER;

pub use script::THEME_JS;

static GENERATION: AtomicU64 = AtomicU64::new(0);

fn rgb(c: [u8; 3]) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

/// Dark scrim opacity. Deliberately light: the cover color has to glow through. Text contrast is
/// carried locally (panel fill, gradient scrims under text), not by darkening the whole window.
pub fn scrim_dark(mean_luma: f32) -> f32 {
    0.08 + 0.14 * ((mean_luma - 0.08) / 0.5).clamp(0.0, 1.0)
}

/// Gray / white covers have no hue to show, so they get a little more darkening instead.
fn neutral_extra(p: &Palette) -> f32 {
    if p.neutral {
        0.14
    } else {
        0.0
    }
}

/// Frost (light) scrim opacity: dark covers need more white to stay pastel.
pub fn scrim_light(mean_luma: f32) -> f32 {
    0.46 - 0.1 * (mean_luma / 0.5).clamp(0.0, 1.0)
}

/// The data `window.__owTheme.apply` takes; `url` must already be vetted (or empty for a flat tint).
pub fn payload(p: &Palette, url: &str) -> serde_json::Value {
    serde_json::json!({
        "url": url,
        "a": rgb(p.accent),
        "b": rgb(p.accent_2),
        "on": rgb(p.on_accent),
        "atd": rgb(p.accent_text),
        "atl": rgb(p.accent_text_light),
        "sad": format!("{:.3}", scrim_dark(p.mean_luma) + neutral_extra(p)).parse::<f64>().unwrap_or(0.3),
        "sal": format!("{:.3}", scrim_light(p.mean_luma)).parse::<f64>().unwrap_or(0.7),
    })
}

/// JS that applies `palette` (and the blurred cover at `url`, when it is an allowed image host).
pub fn apply_script(p: &Palette, url: &str) -> String {
    let url = if allowed_image_url(url) {
        small_url(url)
    } else {
        String::new()
    };
    let payload = payload(p, &url);
    format!("if(window.__owTheme)window.__owTheme.apply({payload});")
}

pub const CLEAR_SCRIPT: &str = "if(window.__owTheme)window.__owTheme.clear();";

/// Call once near the root.
pub fn use_ambient_theme() {
    let cover = use_memo(|| {
        ctl::QUEUE
            .read()
            .current()
            .and_then(|e| e.track.cover.as_ref().map(|c| c.url.to_string()))
    });
    use_effect(move || {
        // `LOW_POWER` is read so toggling it re-applies (flat tint vs blurred layers).
        let _ = LOW_POWER();
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(url) = cover() else {
            document::eval(CLEAR_SCRIPT);
            return;
        };
        spawn(async move {
            let script = match palette_for(&url).await {
                Ok(p) => apply_script(&p, &url),
                Err(_) => CLEAR_SCRIPT.to_string(),
            };
            if GENERATION.load(Ordering::SeqCst) == generation {
                document::eval(&script);
            }
        });
    });
}

/// Fixed layers behind the whole app (see `tools/tailwind.css`, `.ow-backdrop`).
#[component]
pub fn Backdrop() -> Element {
    rsx! {
        div { class: "ow-backdrop", "aria-hidden": "true",
            div { id: "ow-bd-a", class: "ow-bd-layer" }
            div { id: "ow-bd-b", class: "ow-bd-layer" }
            div { class: "ow-bd-scrim" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::palette::from_pixels;

    #[test]
    fn script_carries_colors_and_a_safe_url() {
        let p = from_pixels(&vec![[255, 200, 0]; 100]);
        let s = apply_script(&p, "https://lh3.googleusercontent.com/x=w544-h544-l90-rj");
        assert!(s.contains("w128-h128"));
        assert!(s.contains("\"a\":\"rgb("));
        assert!(s.starts_with("if(window.__owTheme)window.__owTheme.apply({"));
    }

    #[test]
    fn disallowed_urls_never_reach_the_page() {
        let p = from_pixels(&[[10, 10, 10]; 10]);
        let s = apply_script(&p, "https://evil.example/a.jpg\");alert(1);//");
        assert!(s.contains("\"url\":\"\""));
        assert!(!s.contains("evil"));
    }

    #[test]
    fn scrim_grows_with_brightness_and_stays_in_range() {
        assert!(scrim_dark(0.9) > scrim_dark(0.1));
        assert!((0.05..=0.3).contains(&scrim_dark(0.0)) && (0.05..=0.3).contains(&scrim_dark(1.0)));
        assert!(scrim_light(0.0) > scrim_light(1.0));
    }
}

/// Debug helper for the visual check (`cargo test dump_test_covers -- --ignored`): writes four
/// synthetic covers (yellow, black, white, red, blue, multicolor) as PNGs plus the `apply` payload of each
/// (built from the PNG itself as a data URI) into `$OPENWAVE_COVER_DUMP`.
#[cfg(test)]
mod dump {
    use super::*;
    use crate::backend::palette::from_image_bytes;
    use std::io::Cursor;

    fn png(f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut img = image::RgbImage::new(320, 320);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb(f(x, y));
        }
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn b64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for i in 0..4 {
                if i <= c.len() {
                    s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }

    #[test]
    #[ignore]
    fn dump_test_covers() {
        let Some(dir) = std::env::var_os("OPENWAVE_COVER_DUMP") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let covers: Vec<(&str, Vec<u8>)> = vec![
            (
                "yellow",
                png(|x, y| {
                    // Flat saffron with a dark figure-like blob, like a poster cover.
                    let (dx, dy) = (x as f32 - 150.0, y as f32 - 190.0);
                    if dx * dx / 5000.0 + dy * dy / 14000.0 < 1.0 {
                        [30, 22, 10]
                    } else {
                        [255, 190, 0]
                    }
                }),
            ),
            (
                "black",
                png(|x, y| [(x / 40 + y / 40) as u8 % 3 * 3 + 6; 3]),
            ),
            (
                "white",
                png(|x, y| [250 - (x / 80) as u8, 250 - (y / 80) as u8, 248]),
            ),
            (
                "red",
                png(|x, y| {
                    // Saturated red poster with a dark vignette toward the bottom.
                    let d = (y as f32 / 320.0) * 0.45;
                    [
                        (225.0 * (1.0 - d)) as u8,
                        (30.0 * (1.0 - d)) as u8,
                        (45.0 * (1.0 - d)) as u8 + (x / 64) as u8,
                    ]
                }),
            ),
            (
                "blue",
                png(|x, y| {
                    let d = (x as f32 / 320.0) * 0.35;
                    [
                        (25.0 * (1.0 - d)) as u8,
                        (80.0 * (1.0 - d)) as u8 + (y / 64) as u8,
                        (235.0 * (1.0 - d)) as u8,
                    ]
                }),
            ),
            (
                "multicolor",
                png(|x, y| match (x / 107, y / 107) {
                    (0, 0) => [225, 40, 50],
                    (1, 0) => [40, 190, 90],
                    (2, 0) => [40, 90, 235],
                    (0, 1) => [250, 215, 40],
                    (1, 1) => [170, 60, 210],
                    (2, 1) => [20, 190, 210],
                    _ => [245, 130, 30],
                }),
            ),
        ];
        let mut out = serde_json::Map::new();
        for (name, bytes) in covers {
            std::fs::write(dir.join(format!("{name}.png")), &bytes).unwrap();
            let p = from_image_bytes(&bytes).unwrap();
            let url = format!("data:image/png;base64,{}", b64(&bytes));
            out.insert(
                name.into(),
                serde_json::json!({ "palette": p, "payload": payload(&p, &url) }),
            );
        }
        std::fs::write(
            dir.join("covers.json"),
            serde_json::to_vec_pretty(&out).unwrap(),
        )
        .unwrap();
    }
}
