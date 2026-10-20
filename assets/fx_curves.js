/* OpenWave effect math that needs no Web Audio: distortion curve, loudness levelling and the
 * equalizer layout. Pure, so tests/js/fx_curves.test.mjs can check it under Node.
 */
(function (root, factory) {
  var api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.owFxCurves = api;
})(typeof self !== 'undefined' ? self : this, function () {
  /* Ids and order match EQ_FREQS in src/player/effects.rs. */
  var EQ_FREQS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
  var LEVEL_TARGET_DB = -17;
  var LEVEL_RANGE_DB = 9;
  var SILENCE_DB = -50;

  function eqType(i) { return i === 0 ? 'lowshelf' : i === EQ_FREQS.length - 1 ? 'highshelf' : 'peaking'; }

  /* Soft clip (1+k)x / (1+k|x|); drive 0..1. */
  function shape(x, drive) {
    var k = drive * 50;
    return (1 + k) * x / (1 + k * Math.abs(x));
  }

  function curve(drive, n) {
    var out = new Float32Array(n);
    for (var i = 0; i < n; i++) out[i] = shape(i * 2 / (n - 1) - 1, drive);
    return out;
  }

  /* Gain that brings a mid-level signal (0.25) back to where it was before the clipper. */
  function makeup(drive) { return drive > 0 ? 0.25 / shape(0.25, drive) : 1; }

  function rmsDb(samples) {
    var sum = 0;
    for (var i = 0; i < samples.length; i++) sum += samples[i] * samples[i];
    var rms = Math.sqrt(sum / Math.max(1, samples.length));
    return rms > 0 ? 20 * Math.log10(rms) : -Infinity;
  }

  /* Next smoothed loudness (dB) and the gain (linear) that moves it to the target. Silence keeps
   * the previous estimate so quiet intros and pauses do not pump the volume up. */
  function level(prevDb, nowDb) {
    var est = nowDb < SILENCE_DB ? prevDb : prevDb === null ? nowDb : prevDb + (nowDb - prevDb) * 0.08;
    if (est === null) return { db: null, gain: 1 };
    var adjust = Math.max(-LEVEL_RANGE_DB, Math.min(LEVEL_RANGE_DB, LEVEL_TARGET_DB - est));
    return { db: est, gain: Math.pow(10, adjust / 20) };
  }

  return { EQ_FREQS: EQ_FREQS, eqType: eqType, shape: shape, curve: curve, makeup: makeup, rmsDb: rmsDb, level: level };
});
