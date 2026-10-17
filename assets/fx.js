/* OpenWave sound effects: speed, pitch, reverb and bass boost for the ONE hidden <audio> element.
 *
 * Runs in the same eval as audio.js (it shares `dioxus.send` through window.owAudio) after fx_ir.js and
 * fx_stretch.js. Rust drives it with window.owAudio.cmd({c: 'fx', ...}); the state mirrors
 * src/player/effects.rs.
 *
 * Design
 *  - Nothing is created until an effect is actually non-neutral. With everything neutral and no graph yet
 *    (the default), the <audio> element plays exactly as before: no AudioContext, no extra latency.
 *    (An element cannot be taken back out of Web Audio once routed, so after the first use the graph stays
 *    and a neutral state simply wires source -> destination.)
 *  - Speed uses the element's own playbackRate with preservesPitch = false (plain resampling: exact,
 *    currentTime/duration stay in TRACK time). "Linked" mode stops there (slowed / nightcore).
 *  - Independent mode: a live-input Signalsmith Stretch worklet shifts the pitch by
 *    pitch - 12*log2(speed) semitones, which cancels the pitch change of the resampling and adds the
 *    requested shift. If the worklet cannot load, the element falls back to preservesPitch = true.
 *  - Reverb: ConvolverNode with a generated impulse response + equal-power dry/wet. Bass: low shelf.
 */
(function () {
  var host = window.owAudio;
  if (!host || !host.el) return;
  if (window.owFx && window.owFx.dispose) window.owFx.dispose();
  var a = host.el;

  var want = { speed: 1, pitch: 0, linked: false, reverb: 'off', mix: 0.3, bass: 0 };
  var g = null;             /* the graph, once built */
  var building = null;      /* Promise while it is being built */
  var failed = false;       /* graph or worklet failed: stay on the native path */
  var token = 0;            /* latest apply() wins */
  var irId = '';            /* impulse response currently loaded in the convolver */
  var BASS_HZ = 120;

  function clamp(v, lo, hi) { return Math.min(hi, Math.max(lo, v)); }
  function neutral(s) { return s.speed === 1 && s.pitch === 0 && (s.reverb === 'off' || s.mix <= 0) && s.bass <= 0; }
  function fail(why) {
    failed = true;
    try { console.warn('owFx: ' + why); } catch (e) { /* ignore */ }
    host.send({ t: 'fx_failed' });
  }

  /* Semitones the worklet must apply so the audible pitch is `pitch` whatever the resampling does
   * (computed in Rust, Fx::shifter_semitones). */
  function compensation(s) { return s.linked ? 0 : s.shift; }
  function usesStretch(s) { return !s.linked && Math.abs(compensation(s)) > 0.005; }
  function rateFor(s) { return clamp(s.speed, 0.5, 2); }

  function build() {
    if (g) return Promise.resolve(g);
    if (building) return building;
    building = new Promise(function (resolve, reject) {
      var Ctx = window.AudioContext || window.webkitAudioContext;
      if (!Ctx) { reject(new Error('no Web Audio')); return; }
      var ctx = new Ctx({ latencyHint: 'playback' });
      var src = ctx.createMediaElementSource(a);
      var n = {
        ctx: ctx, src: src,
        bass: ctx.createBiquadFilter(), pre: ctx.createGain(),
        dry: ctx.createGain(), conv: ctx.createConvolver(), wet: ctx.createGain(),
        master: ctx.createGain(), analyser: ctx.createAnalyser(), stretch: null
      };
      n.bass.type = 'lowshelf'; n.bass.frequency.value = BASS_HZ; n.bass.gain.value = 0;
      n.analyser.fftSize = 2048;
      n.conv.normalize = true;
      g = n;
      ctx.addEventListener('statechange', function () { if (ctx.state === 'suspended' && !a.paused) ctx.resume().catch(function () {}); });
      resolve(n);
    }).catch(function (e) { building = null; fail(String(e && e.message || e)); throw e; });
    return building;
  }

  /* The worklet loads lazily, only when a pitch shift is needed. Failure keeps everything else working. */
  function ensureStretch(n) {
    if (n.stretch) return Promise.resolve(n.stretch);
    if (n.stretchLoading) return n.stretchLoading;
    if (!window.owStretch) return Promise.reject(new Error('stretch library missing'));
    n.stretchLoading = window.owStretch(n.ctx, { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2] })
      .then(function (node) { n.stretch = node; return node; });
    return n.stretchLoading;
  }

  function setIr(n) {
    if (want.reverb === 'off' || want.reverb === irId) return;
    var ir = window.owFxIr && window.owFxIr.build(want.reverb, n.ctx.sampleRate);
    if (!ir) return;
    var buf = n.ctx.createBuffer(2, ir.length, ir.sampleRate);
    buf.copyToChannel(ir.left, 0);
    buf.copyToChannel(ir.right, 1);
    n.conv.buffer = buf;
    irId = want.reverb;
  }

  function wire(n, s, stretchOn) {
    var t = n.ctx.currentTime, reverbOn = s.reverb !== 'off' && s.mix > 0;
    var shape = (neutral(s) && !stretchOn ? 'straight' : 'fx') + (stretchOn ? '+st' : '') + (reverbOn ? '+rv' : '');
    if (shape !== n.shape) {                   /* rewire only when the topology changes (no clicks while dragging) */
      n.shape = shape;
      [n.src, n.bass, n.pre, n.dry, n.conv, n.wet, n.master].forEach(function (x) { try { x.disconnect(); } catch (e) { /* not connected */ } });
      if (n.stretch) { try { n.stretch.disconnect(); } catch (e) { /* not connected */ } }
      if (shape === 'straight') {              /* nothing to do: straight through */
        n.src.connect(n.master);
      } else {
        n.src.connect(n.bass);
        if (stretchOn && n.stretch) { n.bass.connect(n.stretch); n.stretch.connect(n.pre); } else { n.bass.connect(n.pre); }
        n.pre.connect(n.dry); n.dry.connect(n.master);
        if (reverbOn) { n.pre.connect(n.conv); n.conv.connect(n.wet); n.wet.connect(n.master); }
      }
      n.master.connect(n.ctx.destination);
      n.master.connect(n.analyser);
      n.dry.gain.value = n.wet.gain.value = 0; n.bass.gain.value = 0;   /* parameters of unplugged nodes are frozen; restart clean */
    }
    var m = reverbOn ? clamp(s.mix, 0, 1) : 0;     /* equal-power crossfade */
    n.dry.gain.setTargetAtTime(Math.cos(m * Math.PI / 2), t, 0.02);
    n.wet.gain.setTargetAtTime(Math.sin(m * Math.PI / 2), t, 0.02);
    n.bass.gain.setTargetAtTime(clamp(s.bass, 0, 12), t, 0.02);
  }

  function setMedia(rate, keepPitch) {
    try { a.preservesPitch = keepPitch; a.mozPreservesPitch = keepPitch; a.webkitPreservesPitch = keepPitch; } catch (e) { /* ignore */ }
    a.defaultPlaybackRate = rate;   /* loading the next track resets playbackRate to this */
    a.playbackRate = rate;
  }

  function apply(m) {
    var s = {
      speed: clamp(+m.speed || 1, 0.5, 2), pitch: clamp(+m.pitch || 0, -12, 12), linked: !!m.linked,
      shift: clamp(+m.shift || 0, -24, 24),
      reverb: String(m.reverb || 'off'), mix: clamp(+m.mix || 0, 0, 1), bass: clamp(+m.bass || 0, 0, 12)
    };
    want = s;
    var mine = ++token;
    if ((neutral(s) && !g) || failed) {                /* default path: the element alone */
      if (failed && !neutral(s)) setMedia(rateFor(s), true);   /* browser time-stretch fallback, pitch ignored */
      else setMedia(1, true);
      return;
    }
    build().then(function (n) {
      if (mine !== token) return;
      var needStretch = usesStretch(s);
      var ready = needStretch ? ensureStretch(n).catch(function () { n.stretchFailed = true; return null; }) : Promise.resolve(null);
      return ready.then(function (node) {
        if (mine !== token) return;
        var stretchOn = needStretch && !!node;
        if (needStretch && !node) { fail('pitch shifter unavailable'); }
        setIr(n);
        wire(n, s, stretchOn);
        if (n.stretch) n.stretch.schedule(stretchOn ? { active: true, semitones: compensation(s) } : { active: false });
        if (stretchOn) setMedia(rateFor(s), false);
        else if (s.linked) setMedia(rateFor(s), false);
        else setMedia(rateFor(s), needStretch);        /* needStretch && no node: native pitch-preserving fallback */
        if (!a.paused && n.ctx.state !== 'running') n.ctx.resume().catch(function () {});
      });
    }).catch(function () { /* build() already reported */ });
  }

  function onPlay() { if (g && g.ctx.state !== 'running') g.ctx.resume().catch(function () {}); }
  a.addEventListener('play', onPlay);

  /* Read-only diagnostics for tests: RMS of what reaches the speakers, context state, active settings. */
  function probe() {
    var o = { built: !!g, failed: failed, want: want, rate: a.playbackRate, preservesPitch: a.preservesPitch, crossOrigin: a.crossOrigin };
    if (g) {
      var buf = new Float32Array(g.analyser.fftSize);
      g.analyser.getFloatTimeDomainData(buf);
      var sum = 0, peak = 0;
      for (var i = 0; i < buf.length; i++) { sum += buf[i] * buf[i]; peak = Math.max(peak, Math.abs(buf[i])); }
      o.rms = Math.sqrt(sum / buf.length); o.peak = peak; o.ctx = g.ctx.state; o.sampleRate = g.ctx.sampleRate;
      o.stretch = !!g.stretch; o.semitones = usesStretch(want) ? compensation(want) : 0; o.ir = irId;
      o.dry = g.dry.gain.value; o.wet = g.wet.gain.value; o.bassDb = g.bass.gain.value;
      o.baseLatency = g.ctx.baseLatency; o.outputLatency = g.ctx.outputLatency;
    }
    return o;
  }

  window.owFx = {
    apply: apply, probe: probe,
    /* Seconds of delay the pitch shifter adds while it is in the path (diagnostics). */
    latency: function () { return g && g.stretch ? g.stretch.latency() : Promise.resolve(0); },
    dispose: function () { a.removeEventListener('play', onPlay); }
  };
  host.fx = window.owFx;
})();
