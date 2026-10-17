//! "Ses efektleri" defaults inside Oynatma: the reverb, reverb amount, bass boost and the linked
//! speed/pitch switch that the player remembers between runs. Speed and pitch are not here: they
//! always start neutral (see `store::settings`).

use std::sync::Arc;

use dioxus::prelude::*;

use super::rows::{Card, Row, ToggleRow};
use crate::i18n::t;
use crate::store::settings::{FX_BASS_MAX_DB, FX_MIX_MAX};
use crate::store::{ReverbPreset, Settings, SettingsChange};
use crate::ui::{SegmentedControl, Slider, TabItem};

fn reverb_items() -> Vec<TabItem> {
    ReverbPreset::ALL
        .into_iter()
        .map(|p| TabItem::new(p.id(), t(&format!("fx.reverb.{}", p.id()))))
        .collect()
}

#[component]
fn SliderRow(
    #[props(into)] label: String,
    #[props(into)] readout: String,
    value: f64,
    max: f64,
    step: f64,
    big_step: f64,
    on_commit: EventHandler<f64>,
) -> Element {
    let text = format!("{label} {readout}");
    rsx! {
        div { class: "flex items-center gap-4",
            Slider {
                value,
                min: 0.0,
                max,
                step,
                big_step,
                label,
                value_text: text,
                class: "min-w-0 flex-1",
                on_commit: move |v| on_commit.call(v),
            }
            span { class: "w-16 flex-none text-right text-sm font-semibold tabular-nums text-fg", "{readout}" }
        }
    }
}

#[component]
pub fn EffectsCard(settings: Arc<Settings>, on_change: EventHandler<SettingsChange>) -> Element {
    let mix = f64::from(settings.fx_reverb_mix);
    let bass = f64::from(settings.fx_bass_db);
    rsx! {
        div { class: "ow-keep-next mb-2 mt-7 px-1",
            h3 { class: "ow-display text-lg font-bold text-fg", {t("fx.title")} }
            p { class: "mt-0.5 text-sm text-muted", {t("settings.fx.help")} }
        }
        Card {
            Row { label: t("fx.reverb"), stack: true,
                div { class: "flex w-full flex-wrap",
                    SegmentedControl {
                        items: reverb_items(),
                        value: settings.fx_reverb.id(),
                        group: "fx-reverb",
                        class: "ow-seg flex-wrap",
                        aria_label: t("fx.reverb"),
                        on_change: move |v: String| {
                            if let Some(p) = ReverbPreset::from_id(&v) {
                                on_change.call(SettingsChange::FxReverb(p));
                            }
                        },
                    }
                }
            }
            Row { label: t("fx.mix"), stack: true,
                div { class: "w-full",
                    SliderRow {
                        label: t("fx.mix"),
                        readout: format!("{}%", settings.fx_reverb_mix),
                        value: mix,
                        max: f64::from(FX_MIX_MAX),
                        step: 5.0,
                        big_step: 10.0,
                        on_commit: move |v: f64| on_change.call(SettingsChange::FxReverbMix(v.round() as u8)),
                    }
                }
            }
            Row { label: t("fx.bass"), stack: true,
                div { class: "w-full",
                    SliderRow {
                        label: t("fx.bass"),
                        readout: format!("+{} dB", settings.fx_bass_db),
                        value: bass,
                        max: f64::from(FX_BASS_MAX_DB),
                        step: 1.0,
                        big_step: 3.0,
                        on_commit: move |v: f64| on_change.call(SettingsChange::FxBassDb(v.round() as u8)),
                    }
                }
            }
            ToggleRow {
                label: t("fx.linked"),
                help: t("fx.linked.hint"),
                checked: settings.fx_linked,
                on_change: move |v| on_change.call(SettingsChange::FxLinked(v)),
            }
        }
    }
}
