/* OpenWave audio bridge: the ONE hidden <audio> element.
 * Rust runs this script through document::eval; every event is sent back with dioxus.send(object).
 * Commands arrive as window.owAudio.cmd({c: "...", ...}). Nothing here polls: while paused no timer
 * runs, and "timeupdate" (browser driven, ~4 Hz) is the only periodic message while playing.
 */
(function () {
  if (window.owAudio && window.owAudio.dispose) window.owAudio.dispose();

  var a = document.getElementById('ow-audio');
  if (!a) {
    a = document.createElement('audio');
    a.id = 'ow-audio';
    a.preload = 'auto';
    a.setAttribute('aria-hidden', 'true');
    a.style.display = 'none';
    document.body.appendChild(a);
  }

  /* CORS-enabled loads: required by the sound effects (a cross-origin element routed through Web Audio is
   * silent otherwise). The loopback proxy answers with Access-Control-Allow-Origin for this page's origin only. */
  a.crossOrigin = 'anonymous';

  var gen = 0;          /* load generation, echoed in every event so Rust can drop stale ones */
  var suppress = false; /* true between load() and "loadstart": ignore events of the old source */
  var pendingStart = 0;
  var wantPlay = false;
  var lastTime = 0;
  var outputs = -1;

  function send(o) { o.g = gen; try { dioxus.send(o); } catch (e) { /* page is going away */ } }
  function store(k, v) { try { localStorage.setItem(k, String(v)); } catch (e) { /* storage may be blocked */ } }
  function read(k) { try { return localStorage.getItem(k); } catch (e) { return null; } }
  function finite(n) { return typeof n === 'number' && isFinite(n) && n > 0 ? n : null; }
  function time(force) {
    var now = Date.now();
    if (!force && now - lastTime < 240) return;
    lastTime = now;
    send({ t: 'time', pos: a.currentTime || 0, dur: finite(a.duration) });
  }
  function volume() { send({ t: 'vol', v: a.volume, muted: a.muted }); }

  var on = {
    timeupdate: function () { if (!suppress) time(false); },
    durationchange: function () { if (!suppress) time(true); },
    loadedmetadata: function () {
      if (suppress) return;
      if (pendingStart > 0) { try { a.currentTime = pendingStart; } catch (e) { /* not seekable yet */ } }
      pendingStart = 0;
      time(true);
      if (wantPlay) { start(); }
    },
    playing: function () { if (!suppress) send({ t: 'state', s: 'playing' }); },
    pause: function () { if (!suppress && !a.ended) send({ t: 'state', s: 'paused' }); },
    waiting: function () { if (!suppress && !a.paused) send({ t: 'state', s: 'buffering' }); },
    ended: function () { if (!suppress) send({ t: 'state', s: 'ended' }); },
    error: function () {
      if (suppress || !a.getAttribute('src')) return;
      send({ t: 'error', code: a.error ? a.error.code : 0 });
    },
    loadstart: function () { suppress = false; },
    volumechange: volume
  };
  Object.keys(on).forEach(function (k) { a.addEventListener(k, on[k]); });

  function start() {
    var p = a.play();
    if (p && p.catch) p.catch(function (e) {
      if (e && e.name === 'AbortError') return; /* a newer load() replaced this one */
      send({ t: 'error', code: 0 });
    });
  }

  /* Output device removed (headphones unplugged): the number of audio outputs shrinks. */
  function countOutputs() {
    if (!navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return Promise.resolve(-1);
    return navigator.mediaDevices.enumerateDevices().then(function (list) {
      return list.filter(function (d) { return d.kind === 'audiooutput'; }).length;
    }, function () { return -1; });
  }
  function onDevices() {
    countOutputs().then(function (n) {
      if (outputs >= 0 && n >= 0 && n < outputs) send({ t: 'device_lost' });
      if (n >= 0) outputs = n;
    });
  }
  if (navigator.mediaDevices && navigator.mediaDevices.addEventListener) {
    navigator.mediaDevices.addEventListener('devicechange', onDevices);
    countOutputs().then(function (n) { outputs = n; });
  }

  /* Sleep timer: lower the volume to silence over `secs`, pause, then put the volume back. */
  function fadePause(secs) {
    var from = a.volume, steps = 20, i = 0;
    var id = setInterval(function () {
      i++;
      a.volume = from * (1 - i / steps);
      if (i >= steps) { clearInterval(id); a.pause(); a.volume = from; }
    }, Math.max(10, secs * 1000 / steps));
  }

  var saved = parseFloat(read('ow.volume'));
  a.volume = isFinite(saved) ? Math.min(1, Math.max(0, saved)) : 0.8;
  a.muted = read('ow.muted') === 'true';

  window.owAudio = {
    el: a,
    send: function (o) { try { dioxus.send(o); } catch (e) { /* page is going away */ } },
    cmd: function (m) {
      switch (m.c) {
        case 'load':
          gen = m.g; suppress = true; wantPlay = !!m.play; pendingStart = m.start || 0;
          a.pause();
          a.src = m.url;
          a.load();
          break;
        case 'play': start(); break;
        case 'pause': a.pause(); break;
        case 'stop': suppress = true; wantPlay = false; a.pause(); a.removeAttribute('src'); a.load(); break;
        case 'seek': try { a.currentTime = Math.max(0, m.pos); } catch (e) { /* ignore */ } time(true); break;
        case 'volume': a.volume = Math.min(1, Math.max(0, m.v)); store('ow.volume', a.volume); break;
        case 'mute': a.muted = !!m.on; store('ow.muted', a.muted); break;
        case 'hello': volume(); break;
        case 'fx': if (window.owFx) window.owFx.apply(m); break;
        case 'fade_pause': fadePause(m.secs); break;
      }
    },
    dispose: function () {
      Object.keys(on).forEach(function (k) { a.removeEventListener(k, on[k]); });
      if (navigator.mediaDevices && navigator.mediaDevices.removeEventListener) {
        navigator.mediaDevices.removeEventListener('devicechange', onDevices);
      }
    }
  };
  volume();
})();
