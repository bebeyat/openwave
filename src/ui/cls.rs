//! Tiny helpers shared by the kit: class joining and unique element ids.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Joins class fragments, skipping empty ones: `cx(&["a", "", "b"]) == "a b"`.
pub fn cx(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Process-unique DOM id such as `ow-dialog-7`. Call inside `use_hook` to keep it stable.
pub fn next_id(prefix: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!("ow-{prefix}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cx_skips_empty() {
        assert_eq!(cx(&["a", "", "b"]), "a b");
        assert_eq!(cx(&[]), "");
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(next_id("x"), next_id("x"));
    }
}
