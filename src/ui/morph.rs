//! Morphing glyphs. Icons that switch state (play/pause, volume, repeat, ⋯/menu/X, checkbox,
//! download) are drawn from one set of paths per family whose commands match across states, so
//! `Icon` keeps the same SVG nodes when its name changes and CSS animates `d` between them
//! (`.ow-morph` in tailwind.css). Reduced motion and low power turn the animation off; the end
//! state is the same either way.

/// One path of a morphing glyph in one state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub d: &'static str,
    /// Extra inline CSS for this state (opacity, stroke width, colors, dash offset).
    pub style: &'static str,
    /// Drawn with `pathLength=1`, so `stroke-dashoffset` 1 → 0 draws the stroke in.
    pub draw: bool,
    /// Class for a mount animation (e.g. download → done).
    pub class: &'static str,
}

const fn p(d: &'static str) -> Part {
    ps(d, "")
}

const fn ps(d: &'static str, style: &'static str) -> Part {
    Part {
        d,
        style,
        draw: false,
        class: "",
    }
}

const fn drawn(d: &'static str, style: &'static str) -> Part {
    Part {
        d,
        style,
        draw: true,
        class: "",
    }
}

// Play is split at x=13 into two quads so each half becomes one pause bar.
const PLAY: &[Part] = &[
    p("M6 3 L13 7.5 L13 16.5 L6 21 Z"),
    p("M13 7.5 L20 12 L20 12 L13 16.5 Z"),
];
const PAUSE: &[Part] = &[
    p("M6 4 L10 4 L10 20 L6 20 Z"),
    p("M14 4 L18 4 L18 20 L14 20 Z"),
];

const SPEAKER: &str = "M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z";
// The two waves as cubics, so they can bend into the straight strokes of the mute cross.
const WAVE_S: &str = "M16 9 C17.33 10.33 17.33 13.67 16 15";
const WAVE_L: &str = "M19.36 5.64 C22.9 9.2 22.9 14.8 19.36 18.36";
const VOLUME_2: &[Part] = &[p(SPEAKER), p(WAVE_S), p(WAVE_L)];
const VOLUME_1: &[Part] = &[p(SPEAKER), p(WAVE_S), ps(WAVE_S, "opacity:0")];
const VOLUME_X: &[Part] = &[
    p(SPEAKER),
    p("M16.5 9.5 C18.17 11.17 19.83 12.83 21.5 14.5"),
    p("M21.5 9.5 C19.83 11.17 18.17 12.83 16.5 14.5"),
];

const ARROW_R: &str = "m17 2 4 4-4 4";
const LOOP_TOP: &str = "M3 11v-1a4 4 0 0 1 4-4h14";
const ARROW_L: &str = "m7 22-4-4 4-4";
const LOOP_BOTTOM: &str = "M21 13v1a4 4 0 0 1-4 4H3";
const ONE: &str = "M11 10 L12 10 L12 14";
const REPEAT: &[Part] = &[
    p(ARROW_R),
    p(LOOP_TOP),
    p(ARROW_L),
    p(LOOP_BOTTOM),
    drawn(ONE, "stroke-dashoffset:1;opacity:0"),
];
const REPEAT_1: &[Part] = &[
    p(ARROW_R),
    p(LOOP_TOP),
    p(ARROW_L),
    p(LOOP_BOTTOM),
    drawn(ONE, "stroke-dashoffset:0"),
];

// ⋯ dots are zero-length strokes with a 4px round cap (= Lucide's r=1 circles); the menu bars
// and the X are the same three strokes.
const ELLIPSIS: &[Part] = &[
    ps("M5 12 L5 12", "stroke-width:4"),
    ps("M12 12 L12 12", "stroke-width:4"),
    ps("M19 12 L19 12", "stroke-width:4"),
];
const MENU: &[Part] = &[p("M4 6 L20 6"), p("M4 12 L20 12"), p("M4 18 L20 18")];
const CLOSE: &[Part] = &[
    p("M6 6 L18 18"),
    ps("M12 12 L12 12", "opacity:0"),
    p("M6 18 L18 6"),
];

const BOX: &str =
    "M5 3 H19 A2 2 0 0 1 21 5 V19 A2 2 0 0 1 19 21 H5 A2 2 0 0 1 3 19 V5 A2 2 0 0 1 5 3 Z";
const TICK: &str = "M7.5 12.5 L10.5 15.5 L16.5 9";
const CHECKBOX: &[Part] = &[
    ps(BOX, "fill:transparent;stroke:currentColor"),
    drawn(
        TICK,
        "stroke:var(--ow-on-accent);stroke-dashoffset:1;opacity:0",
    ),
];
const CHECKBOX_ON: &[Part] = &[
    ps(BOX, "fill:var(--ow-accent);stroke:var(--ow-accent)"),
    drawn(TICK, "stroke:var(--ow-on-accent);stroke-dashoffset:0"),
];

const TRAY: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4";
const DOWNLOAD: &[Part] = &[p(TRAY), p("M12 3 L12 15"), p("M7 10 L12 15 L17 10")];
// The arrow folds into a tick; on mount it plays from the arrow (keyframes in tailwind.css).
const DOWNLOAD_DONE: &[Part] = &[
    p(TRAY),
    Part {
        d: "M10.5 14.5 L10.5 14.5",
        style: "opacity:0",
        draw: false,
        class: "ow-morph-in-shaft",
    },
    Part {
        d: "M7 11 L10.5 14.5 L17 8",
        style: "",
        draw: false,
        class: "ow-morph-in-head",
    },
];

type Family = &'static [(&'static str, &'static [Part])];

/// Every family: its states share part count and path commands.
const FAMILIES: &[Family] = &[
    &[("play", PLAY), ("pause", PAUSE)],
    &[
        ("volume-2", VOLUME_2),
        ("volume-1", VOLUME_1),
        ("volume-x", VOLUME_X),
    ],
    &[("repeat", REPEAT), ("repeat-1", REPEAT_1)],
    &[("ellipsis", ELLIPSIS), ("menu", MENU), ("x", CLOSE)],
    &[("checkbox", CHECKBOX), ("checkbox-checked", CHECKBOX_ON)],
    &[("download", DOWNLOAD), ("download-done", DOWNLOAD_DONE)],
];

/// The parts of a morphing icon, or `None` for a plain Lucide icon.
pub fn glyph(name: &str) -> Option<&'static [Part]> {
    FAMILIES
        .iter()
        .flat_map(|f| f.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, parts)| *parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Command letters: what CSS needs to match to interpolate two paths.
    fn commands(d: &str) -> String {
        d.chars().filter(|c| c.is_ascii_alphabetic()).collect()
    }

    #[test]
    fn states_of_a_family_can_interpolate() {
        for family in FAMILIES {
            let (first, base) = family[0];
            for (name, parts) in family.iter() {
                assert_eq!(parts.len(), base.len(), "{name} vs {first}: part count");
                for (a, b) in parts.iter().zip(base.iter()) {
                    assert_eq!(commands(a.d), commands(b.d), "{name} vs {first}");
                    assert_eq!(a.draw, b.draw, "{name} vs {first}: draw flag");
                }
            }
        }
    }

    #[test]
    fn names_resolve_and_plain_icons_do_not() {
        for n in [
            "play",
            "pause",
            "volume-x",
            "repeat-1",
            "x",
            "ellipsis",
            "checkbox-checked",
            "download-done",
        ] {
            assert!(glyph(n).is_some(), "{n}");
        }
        assert!(glyph("heart").is_none());
        assert!(glyph("check").is_none());
    }
}
