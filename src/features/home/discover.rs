//! "Discover" rows of Home: what the catalog suggests for the user's language and country (quick
//! picks, new releases, trending songs, charts, top artists) plus its mood chips. Works for any
//! country: the language follows the app, the country the OS region (or, when the OS names none,
//! the connection's country as the server sees it, with world charts). Rows paint from a disk
//! cache first and refresh in the background.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::row::{RowHeading, RowSkeleton, ScrollRow};
use super::sections::TrackCards;
use super::taste_cache::{self as cache};
use crate::backend::api;
use crate::features::tracks::actions::{open_album, open_artist, play_artist, play_collection};
use crate::i18n::{language, t, t_args, Lang};
use crate::model::{Discover, Shelf, ShelfItems};
use crate::store::library::now_ms;
use crate::ui::{Button, ButtonSize, ButtonVariant, CoverShape, MediaCard};

pub const DISCOVER_FILE: &str = "home_discover.json";
/// Rows older than this are fetched again (they still paint while the refresh runs).
const MAX_AGE_MS: u64 = 6 * 60 * 60 * 1000;

/// Language and country the catalog is asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub hl: String,
    pub gl: Option<String>,
}

/// The app language as the catalog spells it, and the OS region's country.
pub fn region() -> Region {
    Region {
        hl: catalog_language(language()).into(),
        gl: os_locale().and_then(|l| country_of(&l)),
    }
}

/// Debug builds only: `OPENWAVE_LOCALE=ja-JP` stands in for the OS locale (to check other regions).
fn os_locale() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Ok(l) = std::env::var("OPENWAVE_LOCALE") {
        return Some(l);
    }
    sys_locale::get_locale()
}

fn catalog_language(lang: Lang) -> &'static str {
    match lang {
        Lang::Pt => "pt-BR",
        Lang::Zh => "zh-CN",
        other => other.code(),
    }
}

/// `tr-TR`, `en_US`, `zh-Hans-CN`, `sr-Latn-RS.UTF-8` → the two-letter country; `None` when the
/// locale names no country (`en`, `C`).
pub fn country_of(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    base.split(['-', '_'])
        .skip(1)
        .find(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(str::to_ascii_uppercase)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscoverCache {
    pub saved_ms: u64,
    pub region: Region,
    pub feed: Discover,
}

impl DiscoverCache {
    fn is_fresh(&self, region: &Region, now_ms: u64) -> bool {
        &self.region == region
            && !self.feed.shelves.is_empty()
            && now_ms.saturating_sub(self.saved_ms) < MAX_AGE_MS
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Loading,
    Ready,
    Failed,
}

/// The main feed, kept while Home is left and reopened.
static FEED: GlobalSignal<Option<DiscoverCache>> = Signal::global(|| None);

#[component]
pub fn DiscoverRows() -> Element {
    let mut status = use_signal(|| Status::Loading);
    let mut nonce = use_signal(|| 0u32);
    // The picked mood: its label and rows (None = the main feed).
    let mut mood = use_signal(|| None::<(String, Option<Discover>)>);
    let wanted = region();
    use_effect(use_reactive!(|wanted| {
        let _ = nonce();
        let dir = api::data_dir();
        if FEED.peek().is_none() {
            let saved: DiscoverCache = cache::load(&dir, DISCOVER_FILE);
            if saved.region == wanted && !saved.feed.shelves.is_empty() {
                *FEED.write() = Some(saved);
            }
        }
        let fresh = FEED
            .peek()
            .as_ref()
            .is_some_and(|c| c.is_fresh(&wanted, now_ms()));
        if fresh && *nonce.peek() == 0 {
            status.set(Status::Ready);
            return;
        }
        status.set(Status::Loading);
        spawn(async move {
            match api::discover(wanted.hl.clone(), wanted.gl.clone(), None).await {
                Ok(feed) if !feed.shelves.is_empty() => {
                    let c = DiscoverCache {
                        saved_ms: now_ms(),
                        region: wanted,
                        feed,
                    };
                    cache::save(&dir, DISCOVER_FILE, &c);
                    *FEED.write() = Some(c);
                    status.set(Status::Ready);
                }
                _ => status.set(Status::Failed),
            }
        });
    }));
    let feed = FEED.read().as_ref().map(|c| c.feed.clone());
    let failed_empty = status() == Status::Failed && feed.is_none();
    if failed_empty {
        return rsx! {
            section { class: "mb-10",
                RowHeading { title: t("home.discover"), id: "ow-home-discover",
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        icon: "refresh-cw",
                        onclick: move |_| nonce += 1,
                        {t("common.retry")}
                    }
                }
                p { class: "text-sm text-muted", {t("home.section_failed")} }
            }
        };
    }
    let Some(feed) = feed else {
        return rsx! {
            section { class: "mb-10", "aria-busy": "true",
                RowHeading { title: t("home.discover"), id: "ow-home-discover" }
                RowSkeleton { count: 6 }
            }
        };
    };
    let picked = mood();
    let shelves = match &picked {
        Some((_, Some(rows))) => rows.shelves.clone(),
        Some((_, None)) => vec![],
        None => feed.shelves.clone(),
    };
    let region = wanted.clone();
    rsx! {
        if !feed.chips.is_empty() {
            div { class: "mb-6 flex gap-2 overflow-x-auto pb-1 [scrollbar-width:none]", role: "group", "aria-label": t("home.moods"),
                for chip in feed.chips.iter() {
                    {
                        let on = picked.as_ref().is_some_and(|(l, _)| *l == chip.label);
                        let (label, params, region) = (chip.label.clone(), chip.params.clone(), region.clone());
                        rsx! {
                            button {
                                key: "{chip.label}",
                                r#type: "button",
                                "aria-pressed": "{on}",
                                class: "inline-flex h-9 flex-none items-center rounded-full px-4 text-sm font-semibold transition-colors duration-(--ow-dur-hover) {chip_tone(on)}",
                                onclick: move |_| {
                                    if on {
                                        mood.set(None);
                                        return;
                                    }
                                    mood.set(Some((label.clone(), None)));
                                    let (label, params, region) = (label.clone(), params.clone(), region.clone());
                                    spawn(async move {
                                        let rows = api::discover(region.hl, region.gl, Some(params)).await;
                                        // Only if this chip is still the picked one.
                                        if mood.peek().as_ref().is_some_and(|(l, _)| *l == label) {
                                            mood.set(rows.ok().map(|r| (label, Some(r))));
                                        }
                                    });
                                },
                                "{chip.label}"
                            }
                        }
                    }
                }
            }
        }
        if matches!(picked, Some((_, None))) {
            section { class: "mb-10", "aria-busy": "true", RowSkeleton { count: 6 } }
        }
        for (i , shelf) in shelves.into_iter().enumerate() {
            ShelfRow { key: "{i}-{shelf.title}", index: i, shelf }
        }
    }
}

fn chip_tone(on: bool) -> &'static str {
    // Same look as the genre chips of "Türlerin".
    if on {
        "ow-accent-fill"
    } else {
        "border border-line text-muted hover:bg-hover hover:text-fg"
    }
}

fn play_label(title: &str) -> String {
    t_args("home.play_card", &[("title", title)])
}

#[component]
fn ShelfRow(index: usize, shelf: Shelf) -> Element {
    let (heading, row) = (
        format!("ow-home-disc-{index}"),
        format!("ow-row-disc-{index}"),
    );
    rsx! {
        section { class: "mb-10", "aria-labelledby": "{heading}",
            RowHeading { title: shelf.title, id: heading.clone(), row: row.clone() }
            match shelf.items {
                ShelfItems::Tracks(tracks) => rsx! { TrackCards { tracks, row_id: row } },
                ShelfItems::Albums(albums) => rsx! {
                    ScrollRow { id: row,
                        for a in albums {
                            {
                                let (open, play) = (a.id.to_string(), a.id.to_string());
                                let by = a.artists.iter().map(|r| r.name.to_string()).collect::<Vec<_>>().join(", ");
                                rsx! {
                                    MediaCard {
                                        key: "{a.id}",
                                        title: a.title.to_string(),
                                        subtitle: by,
                                        cover: a.cover.as_ref().map(|c| c.url.to_string()),
                                        play_label: play_label(&a.title),
                                        on_open: move |_| open_album(&open),
                                        on_play: move |_| play_collection(play.clone()),
                                    }
                                }
                            }
                        }
                    }
                },
                ShelfItems::Playlists(lists) => rsx! {
                    ScrollRow { id: row,
                        for p in lists {
                            {
                                let (open, play) = (p.id.to_string(), p.id.to_string());
                                rsx! {
                                    MediaCard {
                                        key: "{p.id}",
                                        title: p.title.to_string(),
                                        subtitle: p.author.as_ref().map(|a| a.to_string()),
                                        cover: p.cover.as_ref().map(|c| c.url.to_string()),
                                        play_label: play_label(&p.title),
                                        on_open: move |_| open_album(&open),
                                        on_play: move |_| play_collection(play.clone()),
                                    }
                                }
                            }
                        }
                    }
                },
                ShelfItems::Artists(artists) => rsx! {
                    ScrollRow { id: row,
                        for a in artists {
                            {
                                let (open, play) = (a.id.to_string(), a.id.to_string());
                                rsx! {
                                    MediaCard {
                                        key: "{a.id}",
                                        title: a.name.to_string(),
                                        subtitle: t("detail.artist"),
                                        cover: a.cover.as_ref().map(|c| c.url.to_string()),
                                        shape: CoverShape::Round,
                                        play_label: play_label(&a.name),
                                        on_open: move |_| open_artist(&open),
                                        on_play: move |_| play_artist(play.clone()),
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn country_comes_from_any_locale_spelling() {
        assert_eq!(country_of("tr-TR").as_deref(), Some("TR"));
        assert_eq!(country_of("en_US").as_deref(), Some("US"));
        assert_eq!(country_of("zh-Hans-CN").as_deref(), Some("CN"));
        assert_eq!(country_of("pt_BR.UTF-8").as_deref(), Some("BR"));
        assert_eq!(country_of("sr-Latn-RS@latin").as_deref(), Some("RS"));
        assert_eq!(country_of("en"), None);
        assert_eq!(country_of("C"), None);
        assert_eq!(country_of("es-419"), None);
    }

    #[test]
    fn catalog_languages_are_spelled_as_the_catalog_expects() {
        assert_eq!(catalog_language(Lang::Pt), "pt-BR");
        assert_eq!(catalog_language(Lang::Zh), "zh-CN");
        assert_eq!(catalog_language(Lang::Tr), "tr");
    }

    #[test]
    fn cache_is_fresh_only_for_the_same_region_and_recent() {
        let tr = Region {
            hl: "tr".into(),
            gl: Some("TR".into()),
        };
        let shelf = Shelf {
            title: "x".into(),
            items: ShelfItems::Tracks(vec![]),
        };
        let c = DiscoverCache {
            saved_ms: 1_000,
            region: tr.clone(),
            feed: Discover {
                chips: vec![],
                shelves: vec![shelf],
            },
        };
        assert!(c.is_fresh(&tr, 1_000 + MAX_AGE_MS - 1));
        assert!(!c.is_fresh(&tr, 1_000 + MAX_AGE_MS));
        assert!(!c.is_fresh(
            &Region {
                hl: "en".into(),
                gl: Some("TR".into())
            },
            1_001
        ));
        assert!(!DiscoverCache::default().is_fresh(&Region::default(), 0));
    }
}
