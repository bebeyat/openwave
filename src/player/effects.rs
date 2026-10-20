//! Sound effects: speed, pitch, reverb, bass boost, equalizer, distortion, loudness levelling and
//! crossfade.
//!
//! Pure model (`Fx`, quick presets, semitone math) plus a thin bridge to `assets/fx.js`, which does
//! the DSP in the single hidden `<audio>` element's Web Audio graph. Durations and positions stay in
//! *track time* at any speed: the element's `currentTime`/`duration` are media time, so the seek bar,
//! lyrics and the saved position need no conversion.
//!
//! Persistence: everything but speed and pitch lives in `settings.json`. Speed and
//! pitch are deliberately not saved: every launch starts at normal speed and pitch.

use std::cell::Cell;
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use serde_json::{json, Value};

use super::controller::{self, Status};
use super::{engine, media_keys};
use crate::store::settings::{
    clamp_eq, EQ_BANDS, EQ_MAX_DB, FX_BASS_MAX_DB, FX_DRIVE_MAX, FX_FADE_MAX, FX_MIX_MAX,
};
use crate::store::{self, ReverbPreset, Settings, SettingsChange};

/// The scripts of the effects, appended to the audio bridge in one eval (order matters).
pub const SCRIPTS: &str = concat!(
    include_str!("../../assets/fx_ir.js"),
    "\n",
    include_str!("../../assets/fx_stretch.js"),
    "\n",
    include_str!("../../assets/fx_curves.js"),
    "\n",
    include_str!("../../assets/fx.js")
);

pub const SPEED_MIN: f64 = 0.5;
pub const SPEED_MAX: f64 = 2.0;
pub const SPEED_STEP: f64 = 0.05;
pub const PITCH_MIN: f64 = -12.0;
pub const PITCH_MAX: f64 = 12.0;
/// While the speed is not 1 the system media overlay would drift (it assumes normal speed), so its
/// position is refreshed at least this often.
const OVERLAY_REFRESH: Duration = Duration::from_secs(2);

/// Ratio of two frequencies `semitones` apart (+12 doubles the frequency).
pub fn semitones_to_ratio(semitones: f64) -> f64 {
    (semitones / 12.0).exp2()
}

/// Inverse of [`semitones_to_ratio`]; non-positive ratios give 0.
pub fn ratio_to_semitones(ratio: f64) -> f64 {
    if ratio > 0.0 {
        12.0 * ratio.log2()
    } else {
        0.0
    }
}

/// Everything the user can change. Immutable: every change returns a new value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fx {
    pub speed: f64,
    /// Manual pitch in semitones (ignored while `linked`).
    pub pitch: f64,
    /// Speed and pitch move together (a plain playback-rate change).
    pub linked: bool,
    pub reverb: ReverbPreset,
    /// Reverb wet/dry in percent.
    pub mix: u8,
    /// Bass boost in dB.
    pub bass: u8,
    /// Equalizer gains in dB (see [`EQ_FREQS`]).
    pub eq: [i8; EQ_BANDS],
    /// Distortion in percent.
    pub drive: u8,
    /// Loudness levelling.
    pub level: bool,
    /// Crossfade length in seconds.
    pub fade: u8,
}

/// Centre frequencies of the equalizer bands (the first is a low shelf, the last a high shelf).
pub const EQ_FREQS: [u32; EQ_BANDS] = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

impl Default for Fx {
    fn default() -> Self {
        Self {
            speed: 1.0,
            pitch: 0.0,
            linked: false,
            reverb: ReverbPreset::Off,
            mix: store::settings::DEFAULT_FX_MIX,
            bass: 0,
            eq: [0; EQ_BANDS],
            drive: 0,
            level: false,
            fade: 0,
        }
    }
}

fn snap(v: f64, lo: f64, hi: f64, step: f64) -> f64 {
    let snapped = (v / step).round() * step;
    // Remove float noise such as 0.8500000000000001.
    ((snapped.clamp(lo, hi)) * 1000.0).round() / 1000.0
}

impl Fx {
    /// Remembered effects from the settings; speed and pitch start neutral.
    pub fn from_settings(s: &Settings) -> Self {
        Self::default().with_saved(s)
    }

    /// The remembered part replaced by `s`; speed and pitch are kept (pitch resets when the linked
    /// switch flips, as in [`Fx::with_linked`]).
    pub fn with_saved(self, s: &Settings) -> Self {
        Self {
            pitch: if s.fx_linked == self.linked {
                self.pitch
            } else {
                0.0
            },
            linked: s.fx_linked,
            reverb: s.fx_reverb,
            mix: s.fx_reverb_mix.min(FX_MIX_MAX),
            bass: s.fx_bass_db.min(FX_BASS_MAX_DB),
            eq: clamp_eq(s.fx_eq),
            drive: s.fx_drive.min(FX_DRIVE_MAX),
            level: s.fx_level,
            fade: s.fx_fade.min(FX_FADE_MAX),
            ..self
        }
    }

    pub fn with_speed(self, speed: f64) -> Self {
        if speed.is_nan() {
            return self;
        }
        Self {
            speed: snap(speed, SPEED_MIN, SPEED_MAX, SPEED_STEP),
            ..self
        }
    }

    pub fn with_pitch(self, pitch: f64) -> Self {
        if pitch.is_nan() {
            return self;
        }
        Self {
            pitch: snap(pitch, PITCH_MIN, PITCH_MAX, 1.0),
            ..self
        }
    }

    /// Linking resets the manual pitch, so unlinking later gives a pure speed change.
    pub fn with_linked(self, linked: bool) -> Self {
        Self {
            linked,
            pitch: 0.0,
            ..self
        }
    }

    pub fn with_reverb(self, reverb: ReverbPreset) -> Self {
        Self { reverb, ..self }
    }

    pub fn with_mix(self, mix: f64) -> Self {
        if mix.is_nan() {
            return self;
        }
        Self {
            mix: mix.clamp(0.0, FX_MIX_MAX as f64).round() as u8,
            ..self
        }
    }

    pub fn with_bass(self, db: f64) -> Self {
        if db.is_nan() {
            return self;
        }
        Self {
            bass: db.clamp(0.0, FX_BASS_MAX_DB as f64).round() as u8,
            ..self
        }
    }

    pub fn with_eq(self, eq: [i8; EQ_BANDS]) -> Self {
        Self {
            eq: clamp_eq(eq),
            ..self
        }
    }

    /// One band changed; out-of-range bands are ignored.
    pub fn with_band(self, band: usize, db: f64) -> Self {
        if band >= EQ_BANDS || db.is_nan() {
            return self;
        }
        let mut eq = self.eq;
        eq[band] = db
            .round()
            .clamp(-f64::from(EQ_MAX_DB), f64::from(EQ_MAX_DB)) as i8;
        Self { eq, ..self }
    }

    pub fn with_drive(self, drive: f64) -> Self {
        if drive.is_nan() {
            return self;
        }
        Self {
            drive: drive.clamp(0.0, f64::from(FX_DRIVE_MAX)).round() as u8,
            ..self
        }
    }

    pub fn with_level(self, level: bool) -> Self {
        Self { level, ..self }
    }

    pub fn with_fade(self, secs: f64) -> Self {
        if secs.is_nan() {
            return self;
        }
        Self {
            fade: secs.clamp(0.0, f64::from(FX_FADE_MAX)).round() as u8,
            ..self
        }
    }

    /// Pitch the listener hears relative to the original, in semitones.
    pub fn effective_pitch(&self) -> f64 {
        if self.linked {
            ratio_to_semitones(self.speed)
        } else {
            self.pitch
        }
    }

    /// Semitones the pitch shifter must apply: the wanted pitch ratio divided by the pitch change that
    /// plain resampling at `speed` already causes. Zero when linked (resampling alone is wanted).
    pub fn shifter_semitones(&self) -> f64 {
        if self.linked {
            0.0
        } else {
            ratio_to_semitones(semitones_to_ratio(self.pitch) / self.speed)
        }
    }

    /// Reverb is audible only with a preset and a non-zero mix.
    pub fn reverb_active(&self) -> bool {
        self.reverb != ReverbPreset::Off && self.mix > 0
    }

    /// No effect colours the sound (what the effects button's dot and Reset are about).
    pub fn sounds_plain(&self) -> bool {
        self.speed == 1.0
            && self.effective_pitch().abs() < 1e-9
            && !self.reverb_active()
            && self.bass == 0
            && self.eq == [0; EQ_BANDS]
            && self.drive == 0
    }

    /// Nothing changes the sound: playback stays exactly as without effects.
    pub fn is_neutral(&self) -> bool {
        self.sounds_plain() && !self.level && self.fade == 0
    }

    /// Message for `assets/fx.js`.
    pub fn to_message(self) -> Value {
        json!({
            "c": "fx",
            "speed": self.speed,
            "pitch": self.pitch,
            "shift": self.shifter_semitones(),
            "linked": self.linked,
            "reverb": self.reverb.id(),
            "mix": f64::from(self.mix) / 100.0,
            "bass": f64::from(self.bass),
            "eq": self.eq,
            "drive": f64::from(self.drive) / 100.0,
            "level": self.level,
            "fade": self.fade,
        })
    }
}

/// A one-tap combination. `key` is the i18n key of its name.
#[derive(Clone, Copy, Debug)]
pub struct QuickPreset {
    pub id: &'static str,
    pub key: &'static str,
    pub fx: Fx,
}

const fn quick(
    id: &'static str,
    key: &'static str,
    speed: f64,
    linked: bool,
    reverb: ReverbPreset,
    mix: u8,
    bass: u8,
) -> QuickPreset {
    QuickPreset {
        id,
        key,
        fx: Fx {
            speed,
            pitch: 0.0,
            linked,
            reverb,
            mix,
            bass,
            eq: [0; EQ_BANDS],
            drive: 0,
            level: false,
            fade: 0,
        },
    }
}

impl Fx {
    /// The remembered part of `other` (everything but speed and pitch).
    fn with_remembered(self, other: &Fx) -> Self {
        Self {
            speed: self.speed,
            pitch: self.pitch,
            ..*other
        }
    }

    /// A quick preset's sound on top of the equalizer, distortion, levelling and crossfade.
    pub fn with_quick(self, q: Fx) -> Self {
        Self {
            speed: q.speed,
            pitch: q.pitch,
            linked: q.linked,
            reverb: q.reverb,
            mix: q.mix,
            bass: q.bass,
            ..self
        }
    }

    /// Back to the original sound; levelling and crossfade are listening preferences and stay.
    pub fn reset(self) -> Self {
        Self {
            level: self.level,
            fade: self.fade,
            ..Self::default()
        }
    }
}

pub const QUICK_PRESETS: [QuickPreset; 4] = [
    quick(
        "normal",
        "fx.quick.normal",
        1.0,
        false,
        ReverbPreset::Off,
        30,
        0,
    ),
    quick(
        "slowed",
        "fx.quick.slowed",
        0.85,
        true,
        ReverbPreset::Hall,
        45,
        3,
    ),
    quick(
        "nightcore",
        "fx.quick.nightcore",
        1.25,
        true,
        ReverbPreset::Off,
        30,
        0,
    ),
    quick(
        "concert",
        "fx.quick.concert",
        1.0,
        false,
        ReverbPreset::Salon,
        35,
        2,
    ),
];

/// Id of the quick preset that equals `fx` (the reverb mix is ignored while the reverb is off).
pub fn matching_quick(fx: &Fx) -> Option<&'static str> {
    QUICK_PRESETS
        .iter()
        .find(|q| {
            let same_mix = !q.fx.reverb_active() && !fx.reverb_active() || q.fx.mix == fx.mix;
            q.fx.speed == fx.speed
                && q.fx.pitch == fx.pitch
                && q.fx.linked == fx.linked
                && q.fx.reverb == fx.reverb
                && same_mix
                && q.fx.bass == fx.bass
        })
        .map(|q| q.id)
}

/// Current effects (shared by the player bar button and the popover).
pub static FX: GlobalSignal<Fx> = Signal::global(|| Fx::from_settings(&store::settings().get()));

/// Change the effects: updates the signal, tells the audio page and remembers the persistent part.
pub fn update(change: impl FnOnce(Fx) -> Fx) {
    let prev = *FX.peek();
    let next = change(prev);
    if next == prev {
        return;
    }
    *FX.write() = next;
    engine::send(next.to_message());
    remember(&next);
}

/// Send the current state to a freshly started audio page (no-op while everything is neutral).
pub fn push_current() {
    let fx = *FX.peek();
    if !fx.is_neutral() {
        engine::send(fx.to_message());
    }
}

fn remember(fx: &Fx) {
    let cur = store::settings().get();
    let saved = Fx::default().with_saved(&cur);
    if saved == saved.with_remembered(fx) {
        return;
    }
    let fx = *fx;
    let saved = store::settings().update(|s| {
        s.apply(SettingsChange::FxReverb(fx.reverb))
            .apply(SettingsChange::FxReverbMix(fx.mix))
            .apply(SettingsChange::FxBassDb(fx.bass))
            .apply(SettingsChange::FxLinked(fx.linked))
            .apply(SettingsChange::FxEq(fx.eq))
            .apply(SettingsChange::FxDrive(fx.drive))
            .apply(SettingsChange::FxLevel(fx.level))
            .apply(SettingsChange::FxFade(fx.fade))
    });
    if let Err(e) = saved {
        eprintln!("settings: could not save the sound effects: {e}");
    }
}

thread_local! {
    static LAST_OVERLAY: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether the system media overlay needs a fresh position now.
pub fn overlay_due(speed: f64, last: Option<Instant>, now: Instant) -> bool {
    speed != 1.0 && last.is_none_or(|t| now.duration_since(t) >= OVERLAY_REFRESH)
}

/// Called with every position tick: keeps the overlay's timeline right when the speed is not 1.
pub fn refresh_overlay(position: f64) {
    let speed = FX.peek().speed;
    let status = *controller::STATUS.peek();
    let now = Instant::now();
    if status != Status::Playing || !overlay_due(speed, LAST_OVERLAY.get(), now) {
        return;
    }
    LAST_OVERLAY.set(Some(now));
    media_keys::update_playback(status, position);
}

// Local-only tests (see build.rs); `include!` keeps rustfmt from looking for the file.
#[cfg(all(test, private_tests))]
mod tests {
    include!("effects_tests.rs");
}
