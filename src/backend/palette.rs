//! Cover palette extraction for the ambient theme.
//!
//! `palette_for(url)` downloads a small version of a cover through the same host allow-list the
//! catalog uses (https, `ytimg.com`, `ggpht.com`, `googleusercontent.com`, no redirects, size cap),
//! decodes it to a 64px thumbnail, quantizes it with median cut and returns a [`Palette`] whose
//! `accent`/`accent_2` are safe to paint buttons with and whose text variants reach WCAG AA on the
//! darkest and the lightest UI surfaces. Results are cached on disk (one small JSON file, bounded).
//! Nothing here panics on bad input: every failure is an `Err` or a neutral palette.

use std::io::Cursor;

use serde::{Deserialize, Serialize};

pub type Rgb = [u8; 3];

const THUMB: u32 = 64;
/// Worst-case luminance of the dark UI surface text sits on (backdrop + scrim + panel fill).
const DARK_SURFACE_LUM: f32 = 0.065;
/// Worst-case luminance of the "frost" (light) surface.
const LIGHT_SURFACE_LUM: f32 = 0.62;
const NEUTRAL_SAT: f32 = 0.14;

/// Colors taken from one cover. All `Rgb` values are sRGB bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub dominant: Rgb,
    pub vibrant: Rgb,
    pub muted: Rgb,
    pub dark: Rgb,
    /// The cover is mostly bright (the UI then adds a stronger scrim).
    pub is_light: bool,
    /// Mean relative luminance of the cover, 0..1.
    pub mean_luma: f32,
    /// Fill color for primary buttons, seek fill and glow (any ink-on-fill pair reaches 4.5:1).
    pub accent: Rgb,
    /// Second stop of the accent gradient (hue shifted).
    pub accent_2: Rgb,
    /// Ink to put on `accent` / `accent_2`: near-black or white, whichever reaches 4.5:1 on both.
    pub on_accent: Rgb,
    /// Accent as *text* on dark surfaces (>= 4.5:1 against the darkest-case surface).
    pub accent_text: Rgb,
    /// Accent as text on the light "frost" surface (>= 4.5:1).
    pub accent_text_light: Rgb,
    /// The cover has no usable hue (grayscale, near white or near black).
    pub neutral: bool,
}

// ---- color math ------------------------------------------------------------------------------

fn lin(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance.
pub fn luminance(c: Rgb) -> f32 {
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio between two luminances.
pub fn contrast_lum(a: f32, b: f32) -> f32 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    contrast_lum(luminance(a), luminance(b))
}

fn to_hsl(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < 1e-6 {
        ((g - b) / d).rem_euclid(6.0)
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s.clamp(0.0, 1.0), l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [q(r), q(g), q(b)]
}

/// Moves lightness (keeping hue and saturation) until `ok(rgb)` holds; `dir` is +1 or -1.
fn push_lightness(c: Rgb, dir: f32, ok: impl Fn(Rgb) -> bool) -> Rgb {
    let (h, s, mut l) = to_hsl(c);
    let mut out = c;
    for _ in 0..80 {
        if ok(out) {
            return out;
        }
        l = (l + dir * 0.01).clamp(0.0, 1.0);
        out = from_hsl(h, s, l);
    }
    // Saturation was the obstacle (e.g. a saturated blue cannot get light enough): wash it out.
    let mut s = s;
    for _ in 0..40 {
        if ok(out) {
            break;
        }
        s = (s - 0.05).max(0.0);
        out = from_hsl(h, s, l);
    }
    out
}

fn text_on_dark(c: Rgb) -> Rgb {
    push_lightness(c, 1.0, |x| {
        contrast_lum(luminance(x), DARK_SURFACE_LUM) >= 4.6
    })
}

fn text_on_light(c: Rgb) -> Rgb {
    push_lightness(c, -1.0, |x| {
        contrast_lum(luminance(x), LIGHT_SURFACE_LUM) >= 4.6
    })
}

const INK_DARK: Rgb = [0x0c, 0x0a, 0x12];
const INK_LIGHT: Rgb = [0xff, 0xff, 0xff];

/// Picks the ink and nudges both gradient stops until both reach 4.5:1 with it.
fn settle_fill(a: Rgb, b: Rgb) -> (Rgb, Rgb, Rgb) {
    let (la, lb) = (luminance(a), luminance(b));
    let ink = if (la + lb) / 2.0 > 0.18 {
        INK_DARK
    } else {
        INK_LIGHT
    };
    let dir = if ink == INK_DARK { 1.0 } else { -1.0 };
    let ok = |x: Rgb| contrast(x, ink) >= 4.6;
    (ink, push_lightness(a, dir, ok), push_lightness(b, dir, ok))
}

// ---- quantizer -------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Swatch {
    rgb: Rgb,
    count: usize,
}

fn average(px: &[Rgb]) -> Rgb {
    let n = px.len().max(1) as u64;
    let mut sum = [0u64; 3];
    for p in px {
        for (s, v) in sum.iter_mut().zip(p) {
            *s += u64::from(*v);
        }
    }
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
}

/// Median cut: split the box with the widest channel range at the median, `depth` times.
fn median_cut(px: &mut [Rgb], depth: u32, out: &mut Vec<Swatch>) {
    if px.is_empty() {
        return;
    }
    let ranges: Vec<u8> = (0..3)
        .map(|c| {
            let (lo, hi) = px
                .iter()
                .fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[c]), hi.max(p[c])));
            hi - lo
        })
        .collect();
    let channel = (0..3).max_by_key(|&c| ranges[c]).unwrap_or(0);
    if depth == 0 || px.len() < 2 || ranges[channel] < 4 {
        out.push(Swatch {
            rgb: average(px),
            count: px.len(),
        });
        return;
    }
    px.sort_unstable_by_key(|p| p[channel]);
    let (a, b) = px.split_at_mut(px.len() / 2);
    median_cut(a, depth - 1, out);
    median_cut(b, depth - 1, out);
}

fn neutral_palette(mean_luma: f32, dominant: Rgb, dark: Rgb, light: Rgb) -> Palette {
    let accent = [0xee, 0xec, 0xf6];
    let accent_2 = [0xa9, 0xa6, 0xc2];
    Palette {
        dominant,
        vibrant: dominant,
        muted: dominant,
        dark,
        is_light: mean_luma > 0.5,
        mean_luma,
        accent,
        accent_2,
        on_accent: INK_DARK,
        accent_text: text_on_dark(light),
        accent_text_light: [0x3b, 0x37, 0x52],
        neutral: true,
    }
}

/// Builds a palette from raw RGB pixels (any count). Never panics; an empty input is neutral black.
pub fn from_pixels(pixels: &[Rgb]) -> Palette {
    if pixels.is_empty() {
        return neutral_palette(0.0, [12, 12, 16], [12, 12, 16], [220, 220, 230]);
    }
    let mean_luma = pixels.iter().map(|p| luminance(*p)).sum::<f32>() / pixels.len() as f32;
    let mut work = pixels.to_vec();
    let mut swatches = Vec::new();
    median_cut(&mut work, 3, &mut swatches);
    swatches.sort_by_key(|s| std::cmp::Reverse(s.count));
    let total = pixels.len() as f32;

    let dominant = swatches[0].rgb;
    let dark = swatches
        .iter()
        .min_by(|a, b| luminance(a.rgb).total_cmp(&luminance(b.rgb)))
        .map_or(dominant, |s| s.rgb);
    let light = swatches
        .iter()
        .max_by(|a, b| luminance(a.rgb).total_cmp(&luminance(b.rgb)))
        .map_or(dominant, |s| s.rgb);

    // Saturation-weighted pick: chroma matters more than area, but a speck never wins.
    let score = |s: &Swatch| {
        let (_, sat, l) = to_hsl(s.rgb);
        let share = s.count as f32 / total;
        let mid = 1.0 - ((l - 0.55).abs() * 1.6).min(1.0);
        sat * (0.25 + mid) * share.sqrt()
    };
    let vibrant_sw = swatches
        .iter()
        .filter(|s| {
            let (_, sat, l) = to_hsl(s.rgb);
            sat >= NEUTRAL_SAT && (0.12..=0.92).contains(&l)
        })
        .max_by(|a, b| score(a).total_cmp(&score(b)));
    let Some(vibrant_sw) = vibrant_sw else {
        return neutral_palette(mean_luma, dominant, dark, light);
    };
    let vibrant = vibrant_sw.rgb;
    let muted = swatches
        .iter()
        .filter(|s| {
            let (_, sat, l) = to_hsl(s.rgb);
            (0.05..=0.5).contains(&sat) && (0.2..=0.8).contains(&l)
        })
        .max_by_key(|s| s.count)
        .map_or(dominant, |s| s.rgb);

    // Fill colors: keep the hue, lift saturation, hold lightness where a glow looks good.
    let (h, s, l) = to_hsl(vibrant);
    let s = s.clamp(0.64, 1.0);
    let l = l.clamp(0.5, 0.68);
    let accent = from_hsl(h, s, l);
    // Second stop: hue shifted ~30 degrees. Yellows and greens turn toward orange instead of lime.
    let shift = if (30.0..170.0).contains(&h) {
        -30.0
    } else {
        32.0
    };
    let accent_2 = from_hsl(h + shift, s, (l + 0.04).min(0.72));
    let (on_accent, accent, accent_2) = settle_fill(accent, accent_2);
    Palette {
        dominant,
        vibrant,
        muted,
        dark,
        is_light: mean_luma > 0.5,
        mean_luma,
        accent_text: text_on_dark(accent),
        accent_text_light: text_on_light(accent),
        accent,
        accent_2,
        on_accent,
        neutral: false,
    }
}

/// Decodes image bytes (JPEG, PNG or WebP) into a palette. Dimensions and memory are bounded.
pub fn from_image_bytes(bytes: &[u8]) -> Result<Palette, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| e.to_string())?;
    let small = img.thumbnail(THUMB, THUMB).to_rgba8();
    // Transparent pixels carry no color information: skip them.
    let pixels: Vec<Rgb> = small
        .pixels()
        .filter(|p| p[3] > 32)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok(from_pixels(&pixels))
}

pub use super::palette_net::{allowed_image_url, palette_for, small_url};
