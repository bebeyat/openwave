//! Sleep timer button and popover (the countdown itself lives in `player::sleep`).

use dioxus::prelude::*;

use super::state::SLEEP_OPEN;
use crate::i18n::t;
use std::time::{Duration, Instant};

use crate::player::sleep::{
    self, countdown_text, parse_minutes, step_minutes, Sleep, MINUTES_DEFAULT, MINUTES_LEFT,
    MINUTES_MAX, MINUTES_MIN, SLEEP,
};
use crate::ui::{Button, ButtonVariant, IconButton, IconButtonSize, TooltipAlign};

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
                h2 { class: "flex-1 text-sm font-bold text-fg", {t("sleep.title")} }
                IconButton {
                    icon: "x",
                    label: t("common.close"),
                    shortcut: "Esc",
                    size: IconButtonSize::Sm,
                    onclick: move |_| close(),
                }
            }
            match active {
                Some(Sleep::At(at)) => rsx! { Running { at } },
                Some(Sleep::EndOfTrack) => rsx! {
                    p { class: "py-3 text-center text-lg font-bold text-fg", {t("sleep.until_end")} }
                    StopButton {}
                },
                None => rsx! { Setup {} },
            }
        }
    }
}

/// The countdown, refreshed twice a second while the popover is open.
#[component]
fn Running(at: Instant) -> Element {
    let mut now = use_signal(Instant::now);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            now.set(Instant::now());
        }
    });
    let left = at.saturating_duration_since(now()).as_secs();
    rsx! {
        p {
            class: "ow-display py-2 text-center text-[40px] font-extrabold leading-none tabular-nums text-fg",
            role: "timer",
            "aria-label": t("sleep.title"),
            "{countdown_text(left)}"
        }
        StopButton {}
    }
}

#[component]
fn StopButton() -> Element {
    rsx! {
        Button {
            variant: ButtonVariant::Secondary,
            icon: "square",
            class: "w-full",
            onclick: move |_| sleep::cancel(),
            {t("sleep.stop")}
        }
    }
}

/// Choosing a length: a stepper (or typed minutes), then Start, or "end of song".
#[component]
fn Setup() -> Element {
    let mut minutes = use_signal(|| MINUTES_LEFT.peek().unwrap_or(MINUTES_DEFAULT));
    let mut text = use_signal(|| minutes.peek().to_string());
    let mut set = move |m: u64| {
        minutes.set(m);
        text.set(m.to_string());
    };
    let start = move || {
        let m = parse_minutes(&text.peek()).unwrap_or(*minutes.peek());
        sleep::after_minutes(m);
    };
    let unit = t("sleep.minutes").replace("{n}", "").trim().to_owned();
    // The field is as wide as its digits, so the number and its unit sit in the middle together.
    let digits = text.read().chars().count().max(1);
    rsx! {
        div {
            class: "flex h-12 items-center rounded-lg border border-line-strong bg-surface-2 px-1 focus-within:outline-2 focus-within:outline-accent-text",
            role: "group",
            "aria-label": t("sleep.title"),
            IconButton {
                icon: "minus",
                label: t("sleep.less"),
                size: IconButtonSize::Sm,
                disabled: minutes() <= MINUTES_MIN,
                onclick: move |_| set(step_minutes(minutes(), false)),
            }
            div { class: "flex flex-1 items-baseline justify-center gap-1",
                input {
                    r#type: "text",
                    inputmode: "numeric",
                    class: "bg-transparent text-center text-xl font-bold tabular-nums text-fg outline-none",
                    style: "width:{digits}ch",
                    "aria-label": t("sleep.title"),
                    value: "{text}",
                    oninput: move |e| {
                        let v: String = e.value().chars().filter(char::is_ascii_digit).take(3).collect();
                        if let Some(m) = parse_minutes(&v) {
                            minutes.set(m);
                        }
                        text.set(v);
                    },
                    onblur: move |_| {
                        let m = *minutes.peek();
                        text.set(m.to_string());
                    },
                    onkeydown: move |e| match e.key() {
                        Key::Enter => start(),
                        Key::ArrowUp => set(step_minutes(minutes(), true)),
                        Key::ArrowDown => set(step_minutes(minutes(), false)),
                        _ => {}
                    },
                }
                span { class: "text-sm text-muted", "{unit}" }
            }
            IconButton {
                icon: "plus",
                label: t("sleep.more"),
                size: IconButtonSize::Sm,
                disabled: minutes() >= MINUTES_MAX,
                onclick: move |_| set(step_minutes(minutes(), true)),
            }
        }
        div { class: "grid grid-cols-2 gap-2",
            Button {
                variant: ButtonVariant::Secondary,
                class: "w-full",
                onclick: move |_| sleep::end_of_track(),
                {t("sleep.end_of_track")}
            }
            Button {
                variant: ButtonVariant::Primary,
                class: "w-full",
                onclick: move |_| start(),
                {t("sleep.start")}
            }
        }
    }
}
