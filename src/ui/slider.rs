//! Slider used for seek and volume.
//!
//! * Pointer: press anywhere on the track, drag (pointer capture keeps the drag alive outside the
//!   control and the window), release to commit.
//! * Keyboard: Arrow = `step` (fine), Shift+Arrow / PageUp / PageDown = `big_step` (coarse),
//!   Home / End = min / max.
//! * `on_input` fires continuously while changing; `on_commit` once per gesture (use it to seek).
//!
//! ```ignore
//! Slider { value: pos(), max: duration(), step: 1.0, big_step: 10.0, label: "Konum",
//!          value_text: "1:12 / 3:45", on_input: move |v| pos.set(v), on_commit: move |v| seek(v) }
//! ```

use std::rc::Rc;

use dioxus::prelude::*;

use super::cls::next_id;

/// Maps a pointer x position to a value, snapped to `step`.
pub fn value_from_pointer(x: f64, left: f64, width: f64, min: f64, max: f64, step: f64) -> f64 {
    if width <= 0.0 || max <= min {
        return min;
    }
    let frac = ((x - left) / width).clamp(0.0, 1.0);
    snap(min + frac * (max - min), min, max, step)
}

pub fn snap(v: f64, min: f64, max: f64, step: f64) -> f64 {
    let v = if step > 0.0 {
        min + ((v - min) / step).round() * step
    } else {
        v
    };
    v.clamp(min, max)
}

/// New value after a key press, or `None` if the key is not handled.
pub fn value_from_key(
    key: &Key,
    shift: bool,
    v: f64,
    min: f64,
    max: f64,
    step: f64,
    big: f64,
) -> Option<f64> {
    let delta = match key {
        Key::ArrowRight | Key::ArrowUp => Some(if shift { big } else { step }),
        Key::ArrowLeft | Key::ArrowDown => Some(-(if shift { big } else { step })),
        Key::PageUp => Some(big),
        Key::PageDown => Some(-big),
        _ => None,
    };
    match (key, delta) {
        (Key::Home, _) => Some(min),
        (Key::End, _) => Some(max),
        (_, Some(d)) => Some(snap(v + d, min, max, step)),
        _ => None,
    }
}

fn percent(v: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        0.0
    } else {
        ((v - min) / (max - min) * 100.0).clamp(0.0, 100.0)
    }
}

#[component]
pub fn Slider(
    value: f64,
    #[props(default = 0.0)] min: f64,
    #[props(default = 100.0)] max: f64,
    #[props(default = 1.0)] step: f64,
    #[props(default = 10.0)] big_step: f64,
    #[props(into)] label: String,
    /// Human-readable value for assistive tech, e.g. "1:12 / 3:45" or "Ses 60%".
    #[props(into, default)]
    value_text: Option<String>,
    #[props(default)] disabled: bool,
    #[props(default)] on_input: EventHandler<f64>,
    #[props(default)] on_commit: EventHandler<f64>,
    #[props(default)] class: String,
) -> Element {
    let id = use_hook(|| next_id("slider"));
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let mut rect = use_signal(|| (0.0_f64, 0.0_f64));
    let mut dragging = use_signal(|| false);
    // While dragging we show our own value so the thumb follows the pointer even if the parent
    // updates asynchronously.
    let mut live = use_signal(|| value);
    // Where the pointer last was: a quick click can be released before the track is measured.
    let mut last_x = use_signal(|| 0.0_f64);
    let shown = if dragging() { live() } else { value };
    let p = percent(shown, min, max);

    let mut apply = move |x: f64, commit: bool| {
        let (left, width) = rect();
        // The track is measured asynchronously on press; until then a move would snap to `min`.
        if width <= 0.0 {
            return;
        }
        let v = value_from_pointer(x, left, width, min, max, step);
        live.set(v);
        on_input.call(v);
        if commit {
            on_commit.call(v);
        }
    };

    let state = if disabled {
        "opacity-50 pointer-events-none"
    } else {
        ""
    };
    rsx! {
        div {
            id: "{id}",
            role: "slider",
            tabindex: if disabled { "-1" } else { "0" },
            "aria-label": "{label}",
            "aria-valuemin": "{min}",
            "aria-valuemax": "{max}",
            "aria-valuenow": "{shown}",
            "aria-valuetext": value_text,
            "aria-orientation": "horizontal",
            "aria-disabled": disabled.then_some("true"),
            "data-dragging": dragging().then_some("true"),
            class: "group relative flex h-9 w-full cursor-pointer touch-none items-center outline-none {state} {class}",
            style: "--p:{p}",
            onmounted: move |e| mounted.set(Some(e.data())),
            onpointerdown: move |e| {
                if disabled {
                    return;
                }
                let x = e.client_coordinates().x;
                let Some(m) = mounted() else { return };
                last_x.set(x);
                dragging.set(true);
                // Route every later pointer event of this press to the slider, even outside the
                // window. A full-screen layer cannot do that, and inside a panel with a backdrop
                // filter `position: fixed` would only cover the panel.
                document::eval(&format!(
                    "var s=document.getElementById('{id}');try{{s&&s.setPointerCapture({})}}catch(_){{}}",
                    e.pointer_id()
                ));
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        rect.set((r.origin.x, r.size.width));
                        // Already released (a quick click): commit where the pointer went up.
                        let released = !*dragging.peek();
                        apply(if released { last_x() } else { x }, released);
                    }
                });
            },
            onpointermove: move |e| {
                if !dragging() {
                    return;
                }
                // A release the slider never heard about (capture refused): the next move comes
                // with no button held, so the gesture ends and commits there.
                let released = e.held_buttons().is_empty();
                last_x.set(e.client_coordinates().x);
                apply(e.client_coordinates().x, released);
                if released {
                    dragging.set(false);
                }
            },
            onpointerup: move |e| {
                if dragging() {
                    last_x.set(e.client_coordinates().x);
                    apply(e.client_coordinates().x, true);
                    dragging.set(false);
                }
            },
            onpointercancel: move |_| dragging.set(false),
            onkeydown: move |e| {
                if disabled {
                    return;
                }
                let shift = e.modifiers().shift();
                if let Some(v) = value_from_key(&e.key(), shift, value, min, max, step, big_step) {
                    e.prevent_default();
                    e.stop_propagation();
                    if (v - value).abs() > f64::EPSILON {
                        on_input.call(v);
                        on_commit.call(v);
                    }
                }
            },
            div { class: "relative h-1 w-full rounded-full bg-press group-hover:h-[6px] group-data-[dragging]:h-[6px] transition-[height] duration-(--ow-dur-hover)",
                div { class: "ow-slider-fill absolute inset-y-0 left-0 rounded-full" }
            }
            div {
                class: "ow-slider-thumb pointer-events-none absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 scale-90 rounded-full transition-transform duration-(--ow-dur-hover) group-hover:scale-110 group-focus-visible:outline-2 group-focus-visible:outline-offset-2 group-focus-visible:outline-accent-text group-data-[dragging]:scale-125",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_maps_and_clamps() {
        assert_eq!(
            value_from_pointer(150.0, 100.0, 100.0, 0.0, 200.0, 1.0),
            100.0
        );
        assert_eq!(value_from_pointer(0.0, 100.0, 100.0, 0.0, 200.0, 1.0), 0.0);
        assert_eq!(
            value_from_pointer(999.0, 100.0, 100.0, 0.0, 200.0, 1.0),
            200.0
        );
        assert_eq!(value_from_pointer(5.0, 0.0, 0.0, 0.0, 10.0, 1.0), 0.0);
    }

    #[test]
    fn snapping_respects_step() {
        assert_eq!(snap(7.4, 0.0, 100.0, 5.0), 5.0);
        assert_eq!(snap(7.6, 0.0, 100.0, 5.0), 10.0);
        assert_eq!(snap(120.0, 0.0, 100.0, 5.0), 100.0);
    }

    #[test]
    fn keys_fine_and_coarse() {
        let f = |k: Key, shift: bool| value_from_key(&k, shift, 50.0, 0.0, 100.0, 1.0, 10.0);
        assert_eq!(f(Key::ArrowRight, false), Some(51.0));
        assert_eq!(f(Key::ArrowRight, true), Some(60.0));
        assert_eq!(f(Key::ArrowLeft, false), Some(49.0));
        assert_eq!(f(Key::PageDown, false), Some(40.0));
        assert_eq!(f(Key::Home, false), Some(0.0));
        assert_eq!(f(Key::End, false), Some(100.0));
        assert_eq!(f(Key::Enter, false), None);
    }
}
