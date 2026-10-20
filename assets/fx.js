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
 *  - Equalizer: ten biquads (fx_curves.js has the layout). Distortion: WaveShaper + makeup gain.
 *  - Levelling: a meter before the `level` gain steers it toward one loudness (fx_curves.level).
 *  - Crossfade: the master gain falls over the last `fade` seconds and rises at the next song's start.
 *  - Visualizer: owFx.viz(true) builds the graph (sound unchanged) and draws into canvas#ow-viz.
 */
(function () {
  var host = window.owAudio;
  if (!host || !host.el) return;
  if (window.owFx && window.owFx.dispose) window.owFx.dispose();
  var a = host.el;

  var C = window.owFxCurves;
  var FLAT = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  var want = { speed: 1, pitch: 0, linked: false, reverb: 'off', mix: 0.3, bass: 0, eq: FLAT, drive: 0, level: false, fade: 0 };
  var vizOn = false, vizRunning = false;
  var levelTimer = 0, levelDb = null;
  var g = null;             /* the graph, once built */
  var building = null;      /* Promise while it is being built */
  var failed = false;       /* graph or worklet failed: stay on the native path */
  var token = 0;            /* latest apply() wins */
  var irId = '';            /* impulse response currently loaded in the convolver */
  var BASS_HZ = 120;

  function clamp(v, lo, hi) { return Math.min(hi, Math.max(lo, v)); }
  function flat(eq) { return eq.every(function (v) { return v === 0; }); }
  function neutral(s) {
    return s.speed === 1 && s.pitch === 0 && (s.reverb === 'off' || s.mix <= 0) && s.bass <= 0 &&
      flat(s.eq) && s.drive <= 0 && !s.level && s.fade <= 0;
  }
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
        master: ctx.createGain(), analyser: ctx.createAnalyser(), stretch: null,
        eq: C.EQ_FREQS.map(function () { return ctx.createBiquadFilter(); }),
        drive: ctx.createWaveShaper(), makeup: ctx.createGain(),
        level: ctx.createGain(), meter: ctx.createAnalyser()
      };
      n.bass.type = 'lowshelf'; n.bass.frequency.value = BASS_HZ; n.bass.gain.value = 0;
      n.eq.forEach(function (f, i) { f.type = C.eqType(i); f.frequency.value = C.EQ_FREQS[i]; f.Q.value = 1.4; f.gain.value = 0; });
      n.drive.oversample = '4x';
      n.meter.fftSize = 2048;
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
    var driveOn = s.drive > 0;
    var colour = !(s.speed === 1 && s.pitch === 0 && !reverbOn && s.bass <= 0 && flat(s.eq) && !driveOn && !s.level);
    var shape = (colour || stretchOn ? 'fx' : 'straight') + (stretchOn ? '+st' : '') + (reverbOn ? '+rv' : '') + (driveOn ? '+dr' : '');
    if (shape !== n.shape) {                   /* rewire only when the topology changes (no clicks while dragging) */
      n.shape = shape;
      [n.src, n.bass, n.pre, n.dry, n.conv, n.wet, n.master, n.drive, n.makeup, n.level].concat(n.eq)
        .forEach(function (x) { try { x.disconnect(); } catch (e) { /* not connected */ } });
      if (n.stretch) { try { n.stretch.disconnect(); } catch (e) { /* not connected */ } }
      if (shape === 'straight') {              /* nothing to do: straight through */
        n.src.connect(n.master);
      } else {
        var chain = [n.src, n.bass].concat(n.eq);
        if (driveOn) chain.push(n.drive, n.makeup);
        if (stretchOn && n.stretch) chain.push(n.stretch);
        chain.push(n.pre);
        for (var i = 1; i < chain.length; i++) chain[i - 1].connect(chain[i]);
        n.pre.connect(n.meter);
        n.pre.connect(n.dry); n.dry.connect(n.level);
        if (reverbOn) { n.pre.connect(n.conv); n.conv.connect(n.wet); n.wet.connect(n.level); }
        n.level.connect(n.master);
      }
      n.master.connect(n.ctx.destination);
      n.master.connect(n.analyser);
      n.dry.gain.value = n.wet.gain.value = 0; n.bass.gain.value = 0;   /* parameters of unplugged nodes are frozen; restart clean */
      n.eq.forEach(function (f) { f.gain.value = 0; });
    }
    n.eq.forEach(function (f, i) { f.gain.setTargetAtTime(clamp(s.eq[i] || 0, -12, 12), t, 0.02); });
    if (driveOn && n.driveAmt !== s.drive) {
      n.driveAmt = s.drive;
      n.drive.curve = C.curve(s.drive, 2048);
      n.makeup.gain.value = C.makeup(s.drive);
    }
    levelLoop(n, s.level);
    if (s.fade <= 0) n.master.gain.setTargetAtTime(1, t, 0.05);
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
      reverb: String(m.reverb || 'off'), mix: clamp(+m.mix || 0, 0, 1), bass: clamp(+m.bass || 0, 0, 12),
      eq: FLAT.map(function (_, i) { return clamp(+(m.eq && m.eq[i]) || 0, -12, 12); }),
      drive: clamp(+m.drive || 0, 0, 1), level: !!m.level, fade: clamp(+m.fade || 0, 0, 12)
    };
    run(s);
  }

  function run(s) {
    want = s;
    var mine = ++token;
    if ((neutral(s) && !g && !vizOn) || failed) {                /* default path: the element alone */
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

  /* Levelling: ~10 times a second while on and playing. */
  function levelLoop(n, on) {
    if (on && !levelTimer) {
      levelTimer = setInterval(function () {
        if (a.paused) return;
        var buf = new Float32Array(n.meter.fftSize);
        n.meter.getFloatTimeDomainData(buf);
        var r = C.level(levelDb, C.rmsDb(buf));
        levelDb = r.db;
        n.level.gain.setTargetAtTime(r.gain, n.ctx.currentTime, 0.4);
      }, 100);
    } else if (!on && levelTimer) {
      clearInterval(levelTimer); levelTimer = 0; levelDb = null;
      n.level.gain.setTargetAtTime(1, n.ctx.currentTime, 0.1);
    }
  }

  /* Crossfade: the master gain follows the position (fx_curves.fadeGain), so pausing, seeking or
   * changing speed inside a fade can never leave the song silent. */
  function onTime() {
    if (!g) return;
    var target = want.fade > 0 ? C.fadeGain(a.currentTime, a.duration, a.playbackRate, want.fade) : 1;
    g.master.gain.setTargetAtTime(target, g.ctx.currentTime, 0.08);
  }
  a.addEventListener('timeupdate', onTime);
  a.addEventListener('seeked', onTime);

  /* Visualizer: frequency bars in canvas#ow-viz, coloured with the canvas' CSS colour. */
  var BARS = 48;
  function frame() {
    var c = document.getElementById('ow-viz');
    if (!c || !vizOn || !g) { vizRunning = false; return; }
    var dpr = window.devicePixelRatio || 1, w = Math.round(c.clientWidth * dpr), h = Math.round(c.clientHeight * dpr);
    if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
    var bins = new Uint8Array(g.analyser.frequencyBinCount);
    g.analyser.getByteFrequencyData(bins);
    var x2 = c.getContext('2d');
    x2.clearRect(0, 0, w, h);
    x2.fillStyle = getComputedStyle(c).color;
    var gap = 2 * dpr, bw = (w - gap * (BARS - 1)) / BARS, top = bins.length * 0.7;
    for (var i = 0; i < BARS; i++) {
      var lo = Math.floor(2 * Math.pow(top / 2, i / BARS)), hi = Math.max(lo + 1, Math.floor(2 * Math.pow(top / 2, (i + 1) / BARS)));
      var peak = 0;
      for (var j = lo; j < hi && j < bins.length; j++) peak = Math.max(peak, bins[j]);
      var bh = Math.max(dpr, peak / 255 * h);
      x2.fillRect(i * (bw + gap), h - bh, bw, bh);
    }
    requestAnimationFrame(frame);
  }
  function viz(on) {
    vizOn = !!on;
    if (!vizOn || failed) return;
    if (!g) run(want);
    build().then(function () {
      if (vizOn && !vizRunning) { vizRunning = true; requestAnimationFrame(frame); }
    }).catch(function () { /* build() already reported */ });
  }

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
    apply: apply, probe: probe, viz: viz,
    /* Seconds of delay the pitch shifter adds while it is in the path (diagnostics). */
    latency: function () { return g && g.stretch ? g.stretch.latency() : Promise.resolve(0); },
    dispose: function () {
      a.removeEventListener('play', onPlay);
      a.removeEventListener('timeupdate', onTime);
      a.removeEventListener('seeked', onTime);
      if (levelTimer) clearInterval(levelTimer);
      vizOn = false;
    }
  };
  host.fx = window.owFx;
})();
