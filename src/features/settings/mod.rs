//! Ayarlar (`/settings/:section`): a short section list on the left, plain-language rows on the
//! right. Every change is saved at once and applied live (`boot::commit`).

mod about;
mod backup;
pub mod boot;
mod customize;
mod effects;
mod general;
mod integrations;
mod playback;
mod privacy;
mod rows;
mod shortcuts;
mod storage;
mod user_css;
mod wipe;

use dioxus::prelude::*;

use crate::features::shell::pages::Page;
use crate::i18n::t;
use crate::routes::Route;
use crate::store::{self, SettingsChange};
use crate::ui::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    General,
    Customize,
    Playback,
    Lyrics,
    Storage,
    Privacy,
    Shortcuts,
    About,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::General,
        Section::Customize,
        Section::Playback,
        Section::Lyrics,
        Section::Storage,
        Section::Privacy,
        Section::Shortcuts,
        Section::About,
    ];

    /// The route segment (`/settings/<slug>`).
    pub fn slug(self) -> &'static str {
        match self {
            Section::General => "general",
            Section::Customize => "customize",
            Section::Playback => "playback",
            Section::Lyrics => "lyrics",
            Section::Storage => "storage",
            Section::Privacy => "privacy",
            Section::Shortcuts => "shortcuts",
            Section::About => "about",
        }
    }

    /// Unknown slugs fall back to the first section.
    pub fn from_slug(slug: &str) -> Section {
        Section::ALL
            .into_iter()
            .find(|s| s.slug() == slug)
            .unwrap_or(Section::General)
    }

    fn icon(self) -> &'static str {
        match self {
            Section::General => "sliders-horizontal",
            Section::Customize => "palette",
            Section::Playback => "play",
            Section::Lyrics => "mic-vocal",
            Section::Storage => "hard-drive",
            Section::Privacy => "shield-check",
            Section::Shortcuts => "keyboard",
            Section::About => "info",
        }
    }

    fn label(self) -> String {
        t(&format!("settings.section.{}", self.slug()))
    }
}

#[component]
fn SectionList(current: Section) -> Element {
    rsx! {
        nav { "aria-label": t("settings.sections"), class: "ow-card-glass sticky top-4 flex flex-col gap-1 self-start p-2",
            for s in Section::ALL {
                {
                    let active = s == current;
                    let tone = if active {
                        "ow-nav-active font-semibold text-fg"
                    } else {
                        "text-muted hover:bg-hover hover:text-fg"
                    };
                    rsx! {
                        Link {
                            key: "{s.slug()}",
                            to: Route::Settings { section: s.slug().to_string() },
                            "aria-current": active.then_some("page"),
                            class: "relative flex h-11 items-center gap-3 rounded-lg px-3 text-sm transition-colors duration-(--ow-dur-hover) {tone}",
                            if active {
                                span { class: "absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-accent", "aria-hidden": "true" }
                            }
                            Icon { name: s.icon(), size: 18, class: if active { "text-accent-text" } else { "" } }
                            span { class: "truncate", {s.label()} }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn Settings(section: String) -> Element {
    let current = Section::from_slug(&section);
    let mut snapshot = use_signal(|| store::settings().get());
    let on_change = move |change: SettingsChange| snapshot.set(boot::commit(change));
    let settings = snapshot();
    rsx! {
        Page { title: t("settings.title"),
            div { class: "grid grid-cols-[208px_minmax(0,1fr)] gap-6 max-[1099px]:gap-5",
                SectionList { current }
                div { class: "ow-settings-flow min-w-0", "data-settings-section": current.slug(),
                    match current {
                        Section::General => rsx! { general::General { settings, on_change } },
                        Section::Customize => rsx! { customize::Customize { settings, on_change } },
                        Section::Playback => rsx! { playback::Playback { settings, on_change } },
                        Section::Lyrics => rsx! { playback::Lyrics { settings, on_change } },
                        Section::Storage => rsx! { storage::Storage {} },
                        Section::Privacy => rsx! { privacy::Privacy {} },
                        Section::Shortcuts => rsx! { shortcuts::Shortcuts {} },
                        Section::About => rsx! { about::About {} },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip_and_unknown_falls_back() {
        for s in Section::ALL {
            assert_eq!(Section::from_slug(s.slug()), s);
        }
        assert_eq!(Section::from_slug("nope"), Section::General);
        assert_eq!(Section::from_slug(""), Section::General);
    }

    #[test]
    fn every_slug_is_a_documented_route_section() {
        let documented = [
            "general",
            "customize",
            "playback",
            "lyrics",
            "storage",
            "privacy",
            "shortcuts",
            "about",
        ];
        let slugs: Vec<_> = Section::ALL.iter().map(|s| s.slug()).collect();
        assert_eq!(slugs, documented);
    }
}
