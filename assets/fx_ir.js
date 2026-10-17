/* OpenWave reverb impulse responses, generated procedurally (no sample files).
 * Deterministic: the same preset and sample rate always give the same samples (seeded PRNG), so the
 * result is testable (tests/js/fx_ir.test.mjs). Memory-light: one preset at a time is built, on demand.
 *
 * Model: stereo exponentially decaying noise (-60 dB at `secs`), darker over time (one-pole low-pass
 * whose cutoff falls from `bright` to `dark`), a short pre-delay, and a few sparse early reflections.
 */
(function (root, factory) {
  var api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.owFxIr = api;
})(typeof self !== 'undefined' ? self : this, function () {
  /* secs: reverb time; pre: pre-delay (s); bright/dark: low-pass coefficient at start/end (0..1, 1 = open);
   * early: number of early reflections; seed: PRNG seed. Ids match src/player/effects.rs. */
  var PRESETS = {
    room: { secs: 0.45, pre: 0.003, bright: 0.85, dark: 0.35, early: 6, seed: 1013 },
    salon: { secs: 1.2, pre: 0.01, bright: 0.8, dark: 0.25, early: 8, seed: 2027 },
    hall: { secs: 2.2, pre: 0.02, bright: 0.75, dark: 0.18, early: 10, seed: 3041 },
    cathedral: { secs: 4.0, pre: 0.035, bright: 0.65, dark: 0.1, early: 12, seed: 4057 },
    plate: { secs: 1.6, pre: 0, bright: 1.0, dark: 0.5, early: 0, seed: 5077 }
  };

  /* mulberry32: tiny, fast, deterministic. Returns floats in [0, 1). */
  function rng(seed) {
    var s = seed >>> 0;
    return function () {
      s = (s + 0x6d2b79f5) >>> 0;
      var t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  function channel(p, sampleRate, seed) {
    var rand = rng(seed);
    var n = Math.max(1, Math.round(p.secs * sampleRate));
    var pre = Math.round(p.pre * sampleRate);
    var out = new Float32Array(n + pre);
    var lp = 0;
    for (var i = 0; i < n; i++) {
      var t = i / n;                                   /* 0..1 over the tail */
      var env = Math.pow(10, -3 * t);                  /* -60 dB at the end */
      var a = p.bright + (p.dark - p.bright) * t;      /* darker as it decays */
      lp += a * ((rand() * 2 - 1) - lp);
      out[pre + i] = lp * env;
    }
    for (var e = 0; e < p.early; e++) {                /* sparse early reflections inside ~80 ms */
      var at = pre + Math.round((0.004 + rand() * 0.076) * sampleRate);
      if (at < out.length) out[at] += (rand() < 0.5 ? -1 : 1) * (0.9 - 0.7 * (e / p.early));
    }
    var fade = Math.min(out.length, Math.round(0.005 * sampleRate));
    for (var k = 0; k < fade; k++) out[out.length - 1 - k] *= k / fade;
    return out;
  }

  /* Scale so the loudest sample of both channels is 0.9; the convolver normalizes loudness itself. */
  function build(id, sampleRate) {
    var p = PRESETS[id];
    if (!p || !(sampleRate > 0)) return null;
    var left = channel(p, sampleRate, p.seed);
    var right = channel(p, sampleRate, p.seed ^ 0x9e3779b9);
    var peak = 0;
    for (var i = 0; i < left.length; i++) peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]));
    var g = peak > 0 ? 0.9 / peak : 1;
    for (var j = 0; j < left.length; j++) { left[j] *= g; right[j] *= g; }
    return { left: left, right: right, length: left.length, sampleRate: sampleRate };
  }

  return { PRESETS: PRESETS, build: build, ids: Object.keys(PRESETS) };
});
