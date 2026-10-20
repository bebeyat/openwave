//! Rows of the sound effects popover: preset chips, one-line sliders and the speed/pitch link.

use dioxus::prelude::*;

use crate::i18n::t;
use crate::player::effects::{
    self, matching_quick, Fx, PITCH_MAX, PITCH_MIN, QUICK_PRESETS, SPEED_MAX, SPEED_MIN, SPEED_STEP,
};
use crate::store::ReverbPreset;
use crate::ui::{Button, ButtonSize, ButtonVariant, Slider, Switch};

/// "1.25x".
pub fn speed_text(speed: f64) -> String {
    format!("{speed:.2}x")
}

/// "+3", "-5", "0"; one decimal when the value is not a whole number (linked mode).
pub fn pitch_text(semitones: f64) -> String {
    let whole = (semitones - semitones.round()).abs() < 0.05;
    let v = if whole {
        semitones.round()
    } else {
        (semitones * 10.0).round() / 10.0
    };
    if v == 0.0 {
        "0".to_owned()
    } else if whole {
        format!("{v:+.0}")
    } else {
        format!("{v:+.1}")
    }
}

/// Reverb as one amount: 0 turns it off; raising it from off picks the hall preset (a preset
/// chosen elsewhere, e.g. by a quick preset, is kept).
pub fn with_reverb_amount(fx: Fx, amount: f64) -> Fx {
    if amount <= 0.0 {
        return fx.with_reverb(ReverbPreset::Off);
    }
    let fx = if fx.reverb == ReverbPreset::Off {
        fx.with_reverb(ReverbPreset::Hall)
    } else {
        fx
    };
    fx.with_mix(amount)
}

/// The reverb amount shown: 0 while reverb is off, whatever the stored mix.
pub fn reverb_amount(fx: &Fx) -> f64 {
    if fx.reverb == ReverbPreset::Off {
        0.0
    } else {
        f64::from(fx.mix)
    }
}

/// One line: name, slider, readout.
#[component]
pub(super) fn FxRow(
    #[props(into)] label: String,
    #[props(into)] readout: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    big_step: f64,
    #[props(default)] disabled: bool,
    on_input: EventHandler<f64>,
) -> Element {
    let text = format!("{label} {readout}");
    rsx! {
        div { class: "flex items-center gap-3",
            span { class: "w-20 flex-none truncate text-sm text-muted", "{label}" }
            Slider {
                value,
                min,
                max,
                step,
                big_step,
                label,
                value_text: text,
                disabled,
                class: "h-7!",
                on_input: move |v| on_input.call(v),
            }
            span { class: "w-12 flex-none text-right text-xs font-semibold tabular-nums text-fg", "{readout}" }
        }
    }
}

#[component]
pub fn PresetChips(fx: Fx) -> Element {
    let current = matching_quick(&fx);
    rsx! {
        div { class: "flex flex-wrap gap-1.5", role: "group", "aria-label": t("fx.quick"),
            for q in QUICK_PRESETS {
                Button {
                    key: "{q.id}",
                    variant: if current == Some(q.id) { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                    size: ButtonSize::Sm,
                    pressed: Some(current == Some(q.id)),
                    onclick: move |_| effects::update(|f| f.with_quick(q.fx)),
                    {t(q.key)}
                }
            }
        }
    }
}

#[component]
pub fn FxRows(fx: Fx) -> Element {
    let pitch = fx.effective_pitch();
    rsx! {
        div { class: "flex flex-col",
            FxRow {
                label: t("fx.speed"),
                readout: speed_text(fx.speed),
                value: fx.speed,
                min: SPEED_MIN,
                max: SPEED_MAX,
                step: SPEED_STEP,
                big_step: 0.25,
                on_input: move |v| effects::update(|f| f.with_speed(v)),
            }
            FxRow {
                label: t("fx.pitch"),
                readout: pitch_text(pitch),
                value: pitch,
                min: PITCH_MIN,
                max: PITCH_MAX,
                step: 1.0,
                big_step: 3.0,
                disabled: fx.linked,
                on_input: move |v| effects::update(|f| f.with_pitch(v)),
            }
            FxRow {
                label: t("fx.reverb"),
                readout: format!("{}%", reverb_amount(&fx)),
                value: reverb_amount(&fx),
                min: 0.0,
                max: 100.0,
                step: 5.0,
                big_step: 20.0,
                on_input: move |v| effects::update(|f| with_reverb_amount(f, v)),
            }
            FxRow {
                label: t("fx.bass_short"),
                readout: format!("+{} dB", fx.bass),
                value: f64::from(fx.bass),
                min: 0.0,
                max: 12.0,
                step: 1.0,
                big_step: 3.0,
                on_input: move |v| effects::update(|f| f.with_bass(v)),
            }
            FxRow {
                label: t("fx.drive"),
                readout: format!("{}%", fx.drive),
                value: f64::from(fx.drive),
                min: 0.0,
                max: 100.0,
                step: 5.0,
                big_step: 20.0,
                on_input: move |v| effects::update(|f| f.with_drive(v)),
            }
            FxRow {
                label: t("fx.fade"),
                readout: if fx.fade == 0 { t("fx.off") } else { format!("{} s", fx.fade) },
                value: f64::from(fx.fade),
                min: 0.0,
                max: 12.0,
                step: 1.0,
                big_step: 4.0,
                on_input: move |v| effects::update(|f| f.with_fade(v)),
            }
            div { class: "mt-1 flex items-center justify-between gap-3",
                span { id: "ow-fx-level-t", class: "text-sm text-muted", {t("fx.level")} }
                Switch {
                    id: "ow-fx-level",
                    checked: fx.level,
                    labelled_by: "ow-fx-level-t",
                    on_change: move |on| effects::update(|f| f.with_level(on)),
                }
            }
            div { class: "mt-1 flex items-center justify-between gap-3", title: t("fx.linked.hint"),
                span { id: "ow-fx-linked-t", class: "text-sm text-muted", {t("fx.linked")} }
                Switch {
                    id: "ow-fx-linked",
                    checked: fx.linked,
                    labelled_by: "ow-fx-linked-t",
                    on_change: move |on| effects::update(|f| f.with_linked(on)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readouts_are_plain() {
        assert_eq!(speed_text(1.0), "1.00x");
        assert_eq!(speed_text(0.75), "0.75x");
        assert_eq!(pitch_text(0.0), "0");
        assert_eq!(pitch_text(3.0), "+3");
        assert_eq!(pitch_text(-5.0), "-5");
        assert_eq!(pitch_text(2.8), "+2.8");
        assert_eq!(pitch_text(-2.81), "-2.8");
        assert_eq!(pitch_text(0.001), "0");
    }

    #[test]
    fn reverb_amount_turns_reverb_on_and_off() {
        let off = Fx::default().with_reverb(ReverbPreset::Off);
        assert_eq!(reverb_amount(&off), 0.0);
        let on = with_reverb_amount(off, 40.0);
        assert_eq!((on.reverb, on.mix), (ReverbPreset::Hall, 40));
        assert!(on.reverb_active());
        let cathedral = with_reverb_amount(off.with_reverb(ReverbPreset::Cathedral), 30.0);
        assert_eq!(
            cathedral.reverb,
            ReverbPreset::Cathedral,
            "a chosen preset is kept"
        );
        let silenced = with_reverb_amount(on, 0.0);
        assert_eq!(silenced.reverb, ReverbPreset::Off);
        assert_eq!(reverb_amount(&silenced), 0.0);
    }
}
