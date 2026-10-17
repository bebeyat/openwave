//! "Ses efektleri" popover (compact): quick presets, then one line each for speed, pitch, reverb
//! amount and bass boost, opened from a
//! button in the player bar and in the fullscreen player. The model and persistence live in
//! `player::effects`; this file is only the surface.

use dioxus::prelude::*;

use super::effects_parts::{FxRows, PresetChips};
use super::state::FULLSCREEN;
use crate::i18n::t;
use crate::player::effects::{self, Fx, FX};
use crate::ui::{IconButton, IconButtonSize, TooltipAlign, TooltipSide};

const POPOVER_ID: &str = "ow-fx-pop";
pub const BUTTON_ID: &str = "ow-fx-btn";
const RETURN_FOCUS_JS: &str = "setTimeout(function(){var a=document.activeElement;if(!a||a===document.body){var e=document.getElementById('ow-fx-btn')||document.getElementById('ow-fx-btn-fs');if(e)e.focus();}},60);";

pub static FX_OPEN: GlobalSignal<bool> = Signal::global(|| false);

fn close() {
    *FX_OPEN.write() = false;
}

/// The toggle button. `id` differs between the player bar and the fullscreen player so each can get
/// focus back; an accent dot shows that an effect is on.
#[component]
pub fn FxButton(
    #[props(into, default = BUTTON_ID.to_string())] id: String,
    #[props(default)] tooltip_side: TooltipSide,
    #[props(default)] tooltip_align: TooltipAlign,
    #[props(default = IconButtonSize::Sm)] size: IconButtonSize,
) -> Element {
    let open = *FX_OPEN.read();
    let active = !FX.read().is_neutral();
    let label = if active {
        format!("{} ({})", t("fx.title"), t("fx.on"))
    } else {
        t("fx.title")
    };
    rsx! {
        IconButton {
            id,
            icon: "sliders-horizontal",
            label,
            size,
            tooltip_side,
            tooltip_align,
            pressed: Some(open),
            expanded: open,
            haspopup: "dialog",
            indicator: active,
            onclick: move |_| {
                let next = !*FX_OPEN.peek();
                *FX_OPEN.write() = next;
            },
        }
    }
}

#[component]
pub fn FxPopover() -> Element {
    if !*FX_OPEN.read() {
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
    let fx = *FX.read();
    // The fullscreen player has its button at the top right, so the panel hangs below it there.
    let place = if *FULLSCREEN.read() {
        "top-[88px] max-h-[calc(100vh-112px)]"
    } else {
        "bottom-[104px] max-h-[calc(100vh-128px)]"
    };
    rsx! {
        div { class: "fixed inset-0 z-40", onclick: move |_| close() }
        div {
            id: POPOVER_ID,
            role: "dialog",
            tabindex: "-1",
            "data-ow-overlay": "popover",
            "aria-label": t("fx.title"),
            class: "ow-menu-in ow-glass fixed {place} right-4 z-50 flex w-[340px] max-w-[calc(100vw-32px)] flex-col gap-3 overflow-y-auto overflow-x-hidden rounded-lg p-3",
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    e.stop_propagation();
                    close();
                }
            },
            div { class: "flex items-center gap-1",
                h2 { class: "flex-1 text-sm font-bold text-fg", {t("fx.title")} }
                IconButton {
                    icon: "refresh-cw",
                    label: t("fx.reset"),
                    size: IconButtonSize::Sm,
                    disabled: fx.is_neutral(),
                    onclick: move |_| effects::update(|_| Fx::default()),
                }
                IconButton {
                    icon: "x",
                    label: t("common.close"),
                    shortcut: "Esc",
                    size: IconButtonSize::Sm,
                    onclick: move |_| close(),
                }
            }
            PresetChips { fx }
            FxRows { fx }
        }
    }
}
