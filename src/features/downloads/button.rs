//! "Tümünü indir" for a list: a pill while idle, a progress ring with "done/total" while its
//! tracks download (hover turns it into a cancel button), a quiet "downloaded" pill when done.

use dioxus::prelude::*;

use super::{cancel_all, enqueue, list_progress};
use crate::i18n::{t, t_args};
use crate::model::Track;
use crate::ui::Icon;

const RING_R: f64 = 20.0;

/// Stroke offset that leaves `done / total` of the ring drawn.
pub fn ring_offset(done: usize, total: usize) -> f64 {
    let circumference = 2.0 * std::f64::consts::PI * RING_R;
    let share = if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    };
    circumference * (1.0 - share.clamp(0.0, 1.0))
}

#[component]
pub fn DownloadAllButton(tracks: Vec<Track>) -> Element {
    let (done, total, busy) = list_progress(&tracks);
    let pill = "inline-flex h-10 items-center gap-2 rounded-md px-4 text-sm font-semibold text-fg ring-1 ring-line-strong transition-[background-color] duration-(--ow-dur-hover) hover:bg-hover active:bg-press disabled:pointer-events-none disabled:opacity-50";
    if busy {
        let circumference = 2.0 * std::f64::consts::PI * RING_R;
        let offset = ring_offset(done, total);
        let label = t_args(
            "downloads.ring",
            &[("done", &done.to_string()), ("total", &total.to_string())],
        );
        return rsx! {
            button {
                r#type: "button",
                class: "group relative inline-flex h-12 w-12 flex-none items-center justify-center rounded-full text-fg",
                "aria-label": "{label}",
                title: t("downloads.cancel_all"),
                onclick: move |_| cancel_all(),
                svg {
                    class: "absolute inset-0 -rotate-90",
                    view_box: "0 0 48 48",
                    "aria-hidden": "true",
                    circle { cx: "24", cy: "24", r: "{RING_R}", fill: "none", stroke: "var(--ow-border-strong)", stroke_width: "3" }
                    circle {
                        cx: "24",
                        cy: "24",
                        r: "{RING_R}",
                        fill: "none",
                        stroke: "var(--ow-accent)",
                        stroke_width: "3",
                        stroke_linecap: "round",
                        stroke_dasharray: "{circumference}",
                        stroke_dashoffset: "{offset}",
                        style: "transition: stroke-dashoffset var(--ow-dur-panel) var(--ow-ease)",
                    }
                }
                span { class: "relative text-[9px] font-bold leading-none tabular-nums group-hover:hidden group-focus-visible:hidden", "{done}/{total}" }
                span { class: "relative hidden text-danger-text group-hover:block group-focus-visible:block", "aria-hidden": "true",
                    Icon { name: "x", size: 18 }
                }
            }
        };
    }
    if total > 0 && done == total {
        return rsx! {
            span { class: "{pill} text-muted", role: "status",
                Icon { name: "download-done", size: 16 }
                {t("downloads.all_done")}
            }
        };
    }
    rsx! {
        button {
            r#type: "button",
            class: "{pill}",
            disabled: total == 0,
            onclick: move |_| enqueue(&tracks),
            Icon { name: "download", size: 16 }
            {t("downloads.download_all")}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_fills_with_progress() {
        let full = 2.0 * std::f64::consts::PI * RING_R;
        assert!((ring_offset(0, 100) - full).abs() < 1e-9);
        assert!((ring_offset(50, 100) - full / 2.0).abs() < 1e-9);
        assert!(ring_offset(100, 100).abs() < 1e-9);
        assert!(
            (ring_offset(3, 0) - full).abs() < 1e-9,
            "nothing to download draws no arc"
        );
    }
}
