//! Depolama: cache size, limit, clear (with confirmation) and open folder.

use std::sync::Arc;

use dioxus::prelude::*;

use super::rows::{Card, Row, SectionTitle};
use crate::backend::api;
use crate::backend::error::Coded;
use crate::i18n::{t, t_args, t_count};
use crate::store::{Settings, SettingsChange};
use crate::ui::{
    toast_error, toast_success, Button, ButtonVariant, Dialog, DialogSize, Select, SelectOption,
    Spinner,
};

const LIMIT_PRESETS_MB: [u64; 6] = [128, 256, 512, 1024, 2048, 4096];

/// "812 KB", "12.4 MB", "1.2 GB".
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.1} GB", b / (KB * KB * KB))
    }
}

/// The presets, plus the current value when it is not one of them (sorted).
pub fn limit_options(current_mb: u64) -> Vec<u64> {
    let mut list = LIMIT_PRESETS_MB.to_vec();
    if !list.contains(&current_mb) {
        list.push(current_mb);
        list.sort_unstable();
    }
    list
}

fn limit_label(mb: u64) -> String {
    t_args(
        "settings.storage.cache_limit.value",
        &[("mb", &mb.to_string())],
    )
}

#[component]
pub fn Storage(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    let mut info = use_resource(api::cache_info);
    let mut confirm = use_signal(|| false);
    let options: Vec<SelectOption> = limit_options(settings.cache_limit_mb)
        .into_iter()
        .map(|mb| SelectOption::new(mb.to_string(), limit_label(mb)))
        .collect();
    let usage = match &*info.read() {
        None => rsx! { Spinner { size: 18, label: t("common.loading") } },
        Some(Ok(i)) => rsx! {
            span { class: "text-sm text-fg",
                {t_args("settings.storage.cache_used", &[("size", &format_bytes(i.bytes))])}
                " · "
                {t_count("settings.storage.cache_tracks", i.tracks as usize)}
            }
        },
        Some(Err(e)) => rsx! { span { class: "text-sm text-danger-text", {e.user_message()} } },
    };
    rsx! {
        SectionTitle { title: t("settings.section.storage"), intro: t("settings.storage.help"), icon: "hard-drive" }
        Card {
            Row { label: t("settings.storage.cache_title"), help: t("settings.storage.cache_title.help"),
                {usage}
            }
            Row { label: t("settings.storage.cache_limit"), help: t("settings.storage.cache_limit.help"),
                Select {
                    aria_label: t("settings.storage.cache_limit"),
                    options,
                    value: settings.cache_limit_mb.to_string(),
                    class: "w-40",
                    on_change: move |v: String| {
                        if let Ok(mb) = v.parse::<u64>() {
                            on_change.call(SettingsChange::CacheLimitMb(mb));
                        }
                    },
                }
            }
            Row { label: t("settings.storage.folder"), help: t("settings.storage.folder.help"),
                Button {
                    variant: ButtonVariant::Secondary,
                    icon: "folder-open",
                    onclick: move |_| api::open_cache_folder(),
                    {t("settings.storage.open_folder")}
                }
                Button {
                    variant: ButtonVariant::Danger,
                    icon: "trash-2",
                    onclick: move |_| confirm.set(true),
                    {t("settings.storage.clear")}
                }
            }
        }
        Dialog {
            open: confirm(),
            on_close: move |_| confirm.set(false),
            title: t("settings.storage.clear_confirm.title"),
            description: t("settings.storage.clear_confirm.body"),
            size: DialogSize::Sm,
            footer: rsx! {
                Button { variant: ButtonVariant::Ghost, onclick: move |_| confirm.set(false), {t("common.cancel")} }
                Button {
                    variant: ButtonVariant::Danger,
                    onclick: move |_| {
                        confirm.set(false);
                        spawn(async move {
                            match api::clear_cache().await {
                                Ok(()) => toast_success(t("settings.storage.cleared")),
                                Err(e) => toast_error(e.user_message()),
                            }
                            info.restart();
                        });
                    },
                    {t("settings.storage.clear")}
                }
            },
            div {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_are_human_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(2048), "2 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024 + 400 * 1024), "5.4 MB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn limit_options_keep_a_custom_current_value() {
        assert_eq!(limit_options(512), vec![128, 256, 512, 1024, 2048, 4096]);
        assert_eq!(
            limit_options(700),
            vec![128, 256, 512, 700, 1024, 2048, 4096]
        );
    }
}
