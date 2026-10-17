//! Loading and status components: Skeleton, Spinner, ProgressBar, EmptyState, Banner.

use dioxus::prelude::*;

use super::icon::Icon;

/// Placeholder block while content loads. Size it with `class: "h-4 w-40"`.
/// It is decorative (`aria-hidden`): mark the loading region with `aria-busy` instead.
#[component]
pub fn Skeleton(#[props(default)] class: String, #[props(default)] circle: bool) -> Element {
    let shape = if circle { "rounded-full" } else { "rounded-md" };
    rsx! { div { class: "ow-shimmer bg-surface-2 {shape} {class}", "aria-hidden": "true" } }
}

#[component]
pub fn Spinner(
    #[props(default = 20)] size: u32,
    /// Announced to assistive tech, e.g. "Yükleniyor".
    #[props(into, default)]
    label: Option<String>,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        span {
            role: "status",
            "aria-label": label,
            class: "inline-flex text-muted {class}",
            Icon { name: "loader-circle", size, class: "ow-spin" }
        }
    }
}

/// Determinate when `value` is `Some(0..=100)`, indeterminate when `None`.
#[component]
pub fn ProgressBar(
    #[props(default)] value: Option<f64>,
    #[props(into)] label: String,
    #[props(default)] class: String,
) -> Element {
    let pct = value.map(|v| v.clamp(0.0, 100.0));
    rsx! {
        div {
            role: "progressbar",
            "aria-label": "{label}",
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-valuenow": pct.map(|p| format!("{p:.0}")),
            class: "relative h-1.5 w-full overflow-hidden rounded-full bg-press {class}",
            if let Some(p) = pct {
                div {
                    class: "ow-accent-fill h-full rounded-full transition-[width] duration-(--ow-dur-hover) ease-ow",
                    style: "width:{p}%",
                }
            } else {
                div { class: "ow-indeterminate ow-accent-fill absolute inset-y-0 w-2/5 rounded-full" }
            }
        }
    }
}

/// Friendly empty state: icon, title, one sentence on what to do, and the action(s) as children.
#[component]
pub fn EmptyState(
    #[props(into)] icon: String,
    #[props(into)] title: String,
    #[props(into, default)] text: Option<String>,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center gap-3 px-6 py-12 text-center {class}",
            span { class: "flex h-16 w-16 items-center justify-center rounded-full border border-line bg-surface-2 text-accent-text",
                Icon { name: icon, size: 28 }
            }
            h3 { class: "ow-display text-lg font-bold text-fg", "{title}" }
            if let Some(t) = text {
                p { class: "max-w-[420px] text-sm text-muted", "{t}" }
            }
            div { class: "mt-1 flex flex-wrap items-center justify-center gap-2", {children} }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum BannerKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

/// Inline message with icon (never color alone). `children` is the message; `action` is optional.
#[component]
pub fn Banner(
    #[props(default)] kind: BannerKind,
    #[props(into, default)] title: Option<String>,
    #[props(default)] action: Option<Element>,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let (icon, tone, role) = match kind {
        BannerKind::Info => ("info", "text-info", "status"),
        BannerKind::Success => ("circle-check", "text-success", "status"),
        BannerKind::Warning => ("triangle-alert", "text-warning", "status"),
        BannerKind::Error => ("circle-alert", "text-danger-text", "alert"),
    };
    rsx! {
        div { role, class: "ow-panel flex items-start gap-3 rounded-lg p-4 {class}",
            Icon { name: icon, size: 20, class: "mt-0.5 {tone}" }
            div { class: "min-w-0 flex-1",
                if let Some(t) = title {
                    div { class: "text-sm font-semibold text-fg", "{t}" }
                }
                div { class: "text-sm text-muted", {children} }
            }
            if let Some(a) = action {
                div { class: "flex flex-none items-center gap-2", {a} }
            }
        }
    }
}
