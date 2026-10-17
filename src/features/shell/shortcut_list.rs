//! The documented keyboard shortcuts, shared by the player's "?" popover and the Settings page.
//!
//! Each entry holds two i18n keys: what the shortcut does and the key combination to show
//! (`shortcuts.<name>` and `shortcuts.keys.<name>`). Render with `t(def.label)` / `t(def.keys)`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutDef {
    /// i18n key of the description, e.g. `shortcuts.play_pause`.
    pub label: &'static str,
    /// i18n key of the key combination text, e.g. `shortcuts.keys.play_pause`.
    pub keys: &'static str,
}

const fn def(label: &'static str, keys: &'static str) -> ShortcutDef {
    ShortcutDef { label, keys }
}

/// In display order (matches SPEC section 3, "Keyboard").
pub const SHORTCUTS: [ShortcutDef; 11] = [
    def("shortcuts.play_pause", "shortcuts.keys.play_pause"),
    def("shortcuts.seek", "shortcuts.keys.seek"),
    def("shortcuts.prev_next", "shortcuts.keys.prev_next"),
    def("shortcuts.volume", "shortcuts.keys.volume"),
    def("shortcuts.mute", "shortcuts.keys.mute"),
    def("shortcuts.queue", "shortcuts.keys.queue"),
    def("shortcuts.lyrics", "shortcuts.keys.lyrics"),
    def("shortcuts.fullscreen", "shortcuts.keys.fullscreen"),
    def("shortcuts.search", "shortcuts.keys.search"),
    def("shortcuts.like", "shortcuts.keys.like"),
    def("shortcuts.back", "shortcuts.keys.back"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_pair_with_labels() {
        for d in SHORTCUTS {
            let name = d.label.strip_prefix("shortcuts.").expect("label prefix");
            assert_eq!(d.keys, format!("shortcuts.keys.{name}"));
            assert_ne!(
                crate::i18n::t(d.label),
                d.label,
                "missing text for {}",
                d.label
            );
            assert_ne!(
                crate::i18n::t(d.keys),
                d.keys,
                "missing text for {}",
                d.keys
            );
        }
    }
}
