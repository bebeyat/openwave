//! "Beğenilenler", "Geçmiş" and "Bilgisayarım" tabs.

use dioxus::prelude::*;

use super::actions::{
    add_local_files, add_local_folder, go_to_search, play_all, refresh_local_tags, shuffle_all,
};
use super::logic::{day_label, group_by_day, time_label, DayLabel};
use crate::features::tracks::common::IllustratedEmpty;
use crate::features::tracks::state::{library, update_library};
use crate::features::tracks::TrackList;
use crate::i18n::{t, t_args, t_count};
use crate::model::Track;
use crate::store::library::now_ms;
use crate::ui::{toast, Button, ButtonVariant, Dialog, DialogSize, GradientPlayButton, PlaySize};

/// Gradient play button, Shuffle and the song count; `children` go on the right.
#[component]
fn PlayBar(tracks: Vec<Track>, children: Element) -> Element {
    let (a, b) = (tracks.clone(), tracks.clone());
    rsx! {
        div { class: "mb-5 flex flex-wrap items-center gap-3",
            GradientPlayButton { size: PlaySize::Md, label: t("common.play"), onclick: move |_| play_all(&a) }
            Button {
                variant: ButtonVariant::Secondary,
                icon: "shuffle",
                onclick: move |_| shuffle_all(&b),
                {t("common.shuffle")}
            }
            span { class: "ml-2 text-sm font-medium text-muted", {t_count("common.songs", tracks.len())} }
            div { class: "ml-auto flex items-center gap-2", {children} }
        }
    }
}

fn search_button() -> Element {
    rsx! {
        Button { icon: "search", onclick: move |_| go_to_search(), {t("home.action.search")} }
    }
}

#[component]
pub fn LikedTab() -> Element {
    let liked = library().liked.clone();
    rsx! {
        if !liked.is_empty() {
            PlayBar { tracks: liked.clone(),
                crate::features::downloads::DownloadAllButton { tracks: liked.clone() }
            }
        }
        TrackList {
            tracks: liked,
            label: t("library.tab.liked"),
            empty: rsx! {
                IllustratedEmpty {
                    icon: "heart",
                    title: t("library.empty.liked.title"),
                    text: t("library.empty.liked.body"),
                    {search_button()}
                }
            },
        }
    }
}

#[component]
pub fn LocalTab() -> Element {
    let files = library().local_files.clone();
    let list = files.clone();
    let to_refresh = files.clone();
    rsx! {
        if !files.is_empty() {
            PlayBar { tracks: files.clone(),
                Button {
                    variant: ButtonVariant::Ghost,
                    icon: "refresh-cw",
                    onclick: move |_| refresh_local_tags(to_refresh.clone()),
                    {t("library.local.refresh")}
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "folder-open",
                    onclick: move |_| add_local_folder(),
                    {t("library.local.add_folder")}
                }
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "plus",
                    onclick: move |_| add_local_files(),
                    {t("library.local.add_file")}
                }
            }
        }
        TrackList {
            tracks: files,
            show_album: false,
            label: t("library.tab.local"),
            on_remove: move |i: usize| {
                if let Some(track) = list.get(i) {
                    update_library(|l| l.remove_local_file(&track.id));
                    toast(t("library.local.removed"));
                }
            },
            empty: rsx! {
                IllustratedEmpty {
                    icon: "hard-drive",
                    title: t("library.empty.local.title"),
                    text: t("library.empty.local.body"),
                    Button { icon: "plus", onclick: move |_| add_local_files(), {t("library.local.add_file")} }
                    Button { variant: ButtonVariant::Secondary, icon: "folder-open", onclick: move |_| add_local_folder(), {t("library.local.add_folder")} }
                }
            },
        }
    }
}

fn day_text(day: i64, today: i64) -> String {
    match day_label(day, today) {
        DayLabel::Today => t("library.day.today"),
        DayLabel::Yesterday => t("library.day.yesterday"),
        DayLabel::Date { year, month, day } => {
            let now_year = super::logic::civil_from_days(today).0;
            let args = [
                ("day", day.to_string()),
                ("month", t(&format!("library.month.{month}"))),
                ("year", year.to_string()),
            ];
            let args: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let key = if year == now_year {
                "library.date"
            } else {
                "library.date_year"
            };
            t_args(key, &args)
        }
    }
}

#[component]
fn ClearHistoryDialog(open: bool, on_close: EventHandler<()>) -> Element {
    rsx! {
        Dialog {
            open,
            on_close,
            title: t("library.history.confirm_title"),
            description: Some(t("library.history.confirm_body")),
            size: DialogSize::Sm,
            footer: Some(rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| on_close.call(()), {t("common.cancel")} }
                Button {
                    variant: ButtonVariant::Danger,
                    icon: "trash-2",
                    onclick: move |_| {
                        update_library(|l| l.clear_history());
                        toast(t("library.history.cleared"));
                        on_close.call(());
                    },
                    {t("library.history.clear")}
                }
            }),
            span {}
        }
    }
}

#[component]
pub fn HistoryTab() -> Element {
    let lib = library();
    let mut confirm = use_signal(|| false);
    // Minutes east of UTC, asked from the webview (the Rust side has no time-zone database).
    let offset = use_resource(|| async {
        let mut eval = document::eval("dioxus.send(-new Date().getTimezoneOffset())");
        eval.recv::<i32>().await.unwrap_or(0)
    });
    if lib.history.is_empty() {
        return rsx! {
            IllustratedEmpty {
                icon: "history",
                title: t("library.empty.history.title"),
                text: t("library.empty.history.body"),
                {search_button()}
            }
        };
    }
    let Some(east) = *offset.read() else {
        return rsx! {};
    };
    let groups = group_by_day(&lib.history, east);
    let today = super::logic::local_day(now_ms(), east);
    let times: Vec<String> = lib
        .history
        .iter()
        .map(|h| time_label(h.played_at_ms, east))
        .collect();
    let mut taken = 0usize;
    let rows: Vec<(usize, super::logic::DayGroup, Vec<String>)> = groups
        .into_iter()
        .enumerate()
        .map(|(n, g)| {
            let count = g.tracks.len();
            let slice = times[taken..taken + count].to_vec();
            taken += count;
            (n, g, slice)
        })
        .collect();
    rsx! {
        div { class: "mb-1 flex items-center justify-between gap-2",
            span { class: "text-sm font-medium text-muted", {t_count("common.songs", lib.history.len())} }
            Button {
                variant: ButtonVariant::Secondary,
                icon: "trash-2",
                onclick: move |_| confirm.set(true),
                {t("library.history.clear")}
            }
        }
        for (n , g , group_times) in rows {
            {
                let count = g.tracks.len();
                let heading = day_text(g.day, today);
                rsx! {
                    section { key: "{g.day}", class: "mb-2", "aria-labelledby": "ow-day-{g.day}",
                        div { class: "mb-3 mt-8 flex items-baseline gap-3",
                            h2 { id: "ow-day-{g.day}", class: "ow-display text-2xl font-extrabold tracking-tight text-fg", "{heading}" }
                            span { class: "text-sm text-muted", {t_count("common.songs", count)} }
                        }
                        TrackList {
                            tracks: g.tracks,
                            label: heading.clone(),
                            header: n == 0,
                            extra: group_times,
                            extra_label: t("tracks.col.played"),
                        }
                    }
                }
            }
        }
        ClearHistoryDialog { open: confirm(), on_close: move |()| confirm.set(false) }
    }
}
