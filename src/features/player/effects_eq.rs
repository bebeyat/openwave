//! Equalizer part of the sound effects popover: built-in and saved curves, then one line per band.

use dioxus::prelude::*;

use super::effects_parts::FxRow;
use crate::features::settings::boot;
use crate::i18n::t;
use crate::player::effects::{self, Fx, EQ_FREQS};
use crate::store::settings::{EqPreset, EQ_BANDS, EQ_MAX_DB, EQ_NAME_MAX, EQ_PRESETS_MAX};
use crate::store::{self, SettingsChange};
use crate::ui::{Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, TextInput};

/// Built-in curves: (id, i18n key, gains).
pub const EQ_BUILTIN: [(&str, &str, [i8; EQ_BANDS]); 5] = [
    ("flat", "fx.eq.flat", [0; EQ_BANDS]),
    ("bass", "fx.eq.bass", [6, 5, 4, 2, 0, 0, 0, 0, 0, 0]),
    ("vocal", "fx.eq.vocal", [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1]),
    (
        "electronic",
        "fx.eq.electronic",
        [5, 4, 1, 0, -2, 1, 0, 2, 4, 5],
    ),
    ("rock", "fx.eq.rock", [4, 3, 2, 0, -1, -1, 1, 2, 3, 4]),
];

/// "32", "1k", "16k".
pub fn band_label(hz: u32) -> String {
    if hz >= 1000 {
        format!("{}k", hz / 1000)
    } else {
        hz.to_string()
    }
}

/// "+3 dB", "-2 dB", "0 dB".
pub fn db_text(db: i8) -> String {
    if db > 0 {
        format!("+{db} dB")
    } else {
        format!("{db} dB")
    }
}

/// The saved list with `bands` stored under `name` (replacing a preset of the same name).
pub fn with_preset(list: &[EqPreset], name: &str, bands: [i8; EQ_BANDS]) -> Vec<EqPreset> {
    let name: String = name.trim().chars().take(EQ_NAME_MAX).collect();
    let mut out: Vec<EqPreset> = list.iter().filter(|p| p.name != name).cloned().collect();
    out.push(EqPreset { name, bands });
    out
}

fn save_presets(list: Vec<EqPreset>) {
    boot::commit(SettingsChange::EqPresets(list));
}

#[component]
pub fn EqSection(fx: Fx) -> Element {
    let mut open = use_signal(|| fx.eq != [0; EQ_BANDS]);
    let mut naming = use_signal(|| None::<String>);
    let saved = store::settings().get().eq_presets.clone();
    let full = saved.len() >= EQ_PRESETS_MAX;
    rsx! {
        div { class: "flex flex-col gap-2",
            button {
                r#type: "button",
                class: "flex items-center justify-between text-sm font-semibold text-fg",
                "aria-expanded": "{open()}",
                onclick: move |_| open.toggle(),
                {t("fx.eq")}
                span { class: "text-xs text-muted", if open() { "−" } else { "+" } }
            }
            if open() {
                div { class: "flex flex-wrap gap-1.5", role: "group", "aria-label": t("fx.eq"),
                    for (id, key, bands) in EQ_BUILTIN {
                        Button {
                            key: "{id}",
                            variant: if fx.eq == bands { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                            size: ButtonSize::Sm,
                            pressed: Some(fx.eq == bands),
                            onclick: move |_| effects::update(|f| f.with_eq(bands)),
                            {t(key)}
                        }
                    }
                    for p in saved.iter().cloned() {
                        div { key: "u-{p.name}", class: "flex items-center",
                            Button {
                                variant: if fx.eq == p.bands { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                                size: ButtonSize::Sm,
                                pressed: Some(fx.eq == p.bands),
                                onclick: move |_| effects::update(|f| f.with_eq(p.bands)),
                                "{p.name}"
                            }
                            IconButton {
                                icon: "x",
                                label: format!("{} {}", t("common.delete"), p.name),
                                size: IconButtonSize::Sm,
                                onclick: {
                                    let name = p.name.clone();
                                    move |_| {
                                        let list = store::settings().get().eq_presets.clone();
                                        save_presets(list.into_iter().filter(|q| q.name != name).collect());
                                    }
                                },
                            }
                        }
                    }
                }
                if let Some(name) = naming() {
                    div { class: "flex items-center gap-1.5",
                        TextInput {
                            id: "ow-eq-name",
                            aria_label: t("fx.eq.name"),
                            placeholder: t("fx.eq.name"),
                            value: name.clone(),
                            class: "min-w-0 flex-1",
                            oninput: move |v: String| naming.set(Some(v)),
                            onsubmit: move |_| {
                                let name = naming().unwrap_or_default();
                                if !name.trim().is_empty() {
                                    let list = store::settings().get().eq_presets.clone();
                                    save_presets(with_preset(&list, &name, effects::FX.peek().eq));
                                }
                                naming.set(None);
                            },
                        }
                        IconButton {
                            icon: "x",
                            label: t("common.cancel"),
                            size: IconButtonSize::Sm,
                            onclick: move |_| naming.set(None),
                        }
                    }
                } else if !full {
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        class: "self-start",
                        disabled: fx.eq == [0; EQ_BANDS],
                        onclick: move |_| {
                            naming.set(Some(String::new()));
                            document::eval("setTimeout(function(){var e=document.getElementById('ow-eq-name');if(e)e.focus();},30)");
                        },
                        {t("fx.eq.save")}
                    }
                }
                div { class: "flex flex-col",
                    for (i, hz) in EQ_FREQS.into_iter().enumerate() {
                        FxRow {
                            key: "{hz}",
                            label: band_label(hz),
                            readout: db_text(fx.eq[i]),
                            value: f64::from(fx.eq[i]),
                            min: -f64::from(EQ_MAX_DB),
                            max: f64::from(EQ_MAX_DB),
                            step: 1.0,
                            big_step: 3.0,
                            on_input: move |v| effects::update(|f| f.with_band(i, v)),
                        }
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
    fn labels_and_readouts() {
        assert_eq!(band_label(32), "32");
        assert_eq!(band_label(16000), "16k");
        assert_eq!(db_text(3), "+3 dB");
        assert_eq!(db_text(-2), "-2 dB");
        assert_eq!(db_text(0), "0 dB");
    }

    #[test]
    fn saving_a_name_again_replaces_it() {
        let a = with_preset(&[], " Mine ", [1; EQ_BANDS]);
        let b = with_preset(&a, "Mine", [2; EQ_BANDS]);
        assert_eq!(b.len(), 1);
        assert_eq!((b[0].name.as_str(), b[0].bands), ("Mine", [2; EQ_BANDS]));
    }
}
