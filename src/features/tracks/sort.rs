//! Sorting a track list for display. The list keeps its stored order; `order` only maps display
//! rows to stored indices, so removing a row still removes the right song.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::model::Track;
use crate::ui::{Select, SelectOption};
use crate::util::fold;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    /// The stored order (playlist order, newest like first, ...).
    #[default]
    Default,
    Title,
    Artist,
    Album,
    Duration,
}

impl SortBy {
    const ALL: [SortBy; 5] = [
        SortBy::Default,
        SortBy::Title,
        SortBy::Artist,
        SortBy::Album,
        SortBy::Duration,
    ];

    fn id(self) -> &'static str {
        match self {
            SortBy::Default => "default",
            SortBy::Title => "title",
            SortBy::Artist => "artist",
            SortBy::Album => "album",
            SortBy::Duration => "duration",
        }
    }
}

fn text_key(t: &Track, by: SortBy) -> String {
    match by {
        SortBy::Title => fold(&t.title),
        SortBy::Artist => fold(&t.artists_label()),
        SortBy::Album => t.album.as_ref().map(|a| fold(&a.title)).unwrap_or_default(),
        SortBy::Default | SortBy::Duration => String::new(),
    }
}

/// Stored indices in display order. Stable: equal keys keep the stored order.
pub fn order(tracks: &[Track], by: SortBy) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..tracks.len()).collect();
    match by {
        SortBy::Default => {}
        SortBy::Duration => idx.sort_by_key(|&i| tracks[i].duration_ms),
        _ => idx.sort_by_cached_key(|&i| text_key(&tracks[i], by)),
    }
    idx
}

#[component]
pub fn SortControl(value: SortBy, on_change: EventHandler<SortBy>) -> Element {
    let options: Vec<SelectOption> = SortBy::ALL
        .into_iter()
        .map(|s| SelectOption {
            value: s.id().to_owned(),
            label: t(&format!("sort.{}", s.id())),
            icon: None,
            image: None,
        })
        .collect();
    rsx! {
        Select {
            options,
            value: value.id(),
            aria_label: t("sort.label"),
            on_change: move |id: String| {
                if let Some(s) = SortBy::ALL.into_iter().find(|s| s.id() == id) {
                    on_change.call(s);
                }
            },
        }
    }
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("sort_tests.rs");
}
