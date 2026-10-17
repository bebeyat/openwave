//! The OpenWave emblem (the wave of the app icon) for use inside the app. The PNG is an alpha
//! mask, so the mark takes `currentColor` (accent in the top strip, on-accent in a chip).

use std::sync::OnceLock;

use dioxus::prelude::*;

const EMBLEM_PNG: &[u8] = include_bytes!("../../assets/branding/openwave-emblem.png");
/// Width / height of the emblem picture.
const ASPECT: f64 = 285.0 / 128.0;

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// `mask` declarations painting the emblem (built once).
fn mask_style() -> &'static str {
    static STYLE: OnceLock<String> = OnceLock::new();
    STYLE.get_or_init(|| {
        let url = format!("url(data:image/png;base64,{})", base64(EMBLEM_PNG));
        format!(
            "background:currentColor;-webkit-mask:{url} center/contain no-repeat;mask:{url} center/contain no-repeat"
        )
    })
}

/// The emblem, `height` px tall, coloured by the surrounding text colour.
#[component]
pub fn Emblem(height: u32, #[props(default)] class: String) -> Element {
    let width = (f64::from(height) * ASPECT).round();
    rsx! {
        span {
            class: "inline-block flex-none {class}",
            "aria-hidden": "true",
            style: "width:{width}px;height:{height}px;{mask_style()}",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_emblem_is_a_png() {
        assert_eq!(&EMBLEM_PNG[..8], b"\x89PNG\r\n\x1a\n");
        assert!(mask_style().contains("data:image/png;base64,iVBOR"));
    }
}
