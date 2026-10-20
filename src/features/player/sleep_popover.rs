//! Sleep timer button and popover (the countdown itself lives in `player::sleep`).

use dioxus::prelude::*;

use super::state::SLEEP_OPEN;
use crate::i18n::t;
use crate::player::sleep::{self, Sleep, MINUTES_LEFT, MINUTE_CHOICES, SLEEP};
use crate::ui::{Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, TooltipAlign};

const POPOVER_ID: &str = "ow-sleep-pop";
const RETURN_FOCUS_JS: &str = "setTimeout(function(){var a=document.activeElement;if(!a||a===document.body){var e=document.getElementById('ow-sleep-btn');if(e)e.focus();}},60);";

fn close() {
    *SLEEP_OPEN.write() = false;
}

/// "Sleep timer", "Sleep timer (12 min)", "Sleep timer (end of song)".
fn label() -> String {
    let base = t("sleep.title");
    match (*SLEEP.read(), *MINUTES_LEFT.read()) {
        (Some(Sleep::EndOfTrack), _) => format!("{base} ({})", t("sleep.end_of_track")),
        (Some(Sleep::At(_)), Some(m)) => format!(
            "{base} ({})",
            t("sleep.minutes").replace("{n}", &m.to_string())
        ),
        _ => base,
    }
}

#[component]
pub fn SleepButton() -> Element {
    let open = *SLEEP_OPEN.read();
    rsx! {
        IconButton {
            id: "ow-sleep-btn",
            icon: "moon",
            label: label(),
            size: IconButtonSize::Sm,
            pressed: Some(open),
            expanded: open,
            haspopup: "dialog",
            indicator: SLEEP.read().is_some(),
            tooltip_align: TooltipAlign::End,
            onclick: move |_| {
                let next = !*SLEEP_OPEN.peek();
                *SLEEP_OPEN.write() = next;
            },
        }
    }
}

#[component]
pub fn SleepPopover() -> Element {
    if !*SLEEP_OPEN.read() {
        return rsx! {};
    }
    rsx! { Panel {} }
}

#[component]
fn Panel() -> Element {
    use_effect(|| {
        document::eval(&format!("window.ow.focusId('{POPOVER_ID}')"));
    });
    use_drop(|| {
        document::eval(RETURN_FOCUS_JS);
    });
    let active = *SLEEP.read();
    rsx! {
        div { class: "fixed inset-0 z-40", onclick: move |_| close() }
        div {
            id: POPOVER_ID,
            role: "dialog",
            tabindex: "-1",
            "data-ow-overlay": "popover",
            "aria-label": t("sleep.title"),
            class: "ow-menu-in ow-glass fixed bottom-[104px] right-4 z-50 flex w-[300px] max-w-[calc(100vw-32px)] flex-col gap-3 rounded-lg p-3",
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    close();
                }
            },
            div { class: "flex items-center gap-1",
                h2 { class: "flex-1 text-sm font-bold text-fg", {label()} }
                IconButton {
                    icon: "x",
                    label: t("common.close"),
                    shortcut: "Esc",
                    size: IconButtonSize::Sm,
                    onclick: move |_| close(),
                }
            }
            div { class: "flex flex-wrap gap-1.5", role: "group", "aria-label": t("sleep.title"),
                for m in MINUTE_CHOICES {
                    Button {
                        key: "{m}",
                        variant: ButtonVariant::Secondary,
                        size: ButtonSize::Sm,
                        onclick: move |_| {
                            sleep::after_minutes(m);
                            close();
                        },
                        {t("sleep.minutes").replace("{n}", &m.to_string())}
                    }
                }
                Button {
                    variant: if active == Some(Sleep::EndOfTrack) { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                    size: ButtonSize::Sm,
                    pressed: Some(active == Some(Sleep::EndOfTrack)),
                    onclick: move |_| {
                        sleep::end_of_track();
                        close();
                    },
                    {t("sleep.end_of_track")}
                }
            }
            if active.is_some() {
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Sm,
                    onclick: move |_| {
                        sleep::cancel();
                        close();
                    },
                    {t("sleep.cancel")}
                }
            }
        }
    }
}
