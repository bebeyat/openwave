/* Karaoke fill for synced lyrics. Words of the active line are .ow-w spans inside
 * .ow-lyric[aria-current], with their fill span in ms as data-s / data-e. Each frame sets --p
 * (0..1) on those words from the <audio> clock, so seeking, pausing and speed changes stay in
 * sync without any message to Rust. The frame loop only runs while audio plays and a lyrics view
 * has an active line; low power and reduced motion update on "timeupdate" only, word by word.
 */
(function () {
  if (window.__owLyricFill) window.__owLyricFill();
  var a = document.getElementById('ow-audio');
  if (!a) return;
  var LEAD_MS = 100; /* the same small lead the line highlight uses, so words feel on the beat */
  var raf = 0;

  function reduce() {
    return document.documentElement.hasAttribute('data-low-power') ||
      (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  }

  function paint() {
    var words = document.querySelectorAll('.ow-lyric[aria-current] .ow-w');
    var t = (a.currentTime || 0) * 1000 + LEAD_MS;
    var step = reduce(), pending = false;
    for (var i = 0; i < words.length; i++) {
      var w = words[i], s = +w.dataset.s, e = +w.dataset.e;
      var p = t <= s ? 0 : t >= e ? 1 : step ? 1 : (t - s) / (e - s);
      var v = p.toFixed(3);
      if (w.style.getPropertyValue('--p') !== v) w.style.setProperty('--p', v);
      if (p < 1) pending = true;
    }
    return pending;
  }

  function frame() {
    raf = 0;
    /* Nothing left to fill (line sung, view closed): sleep until an event or a new active line. */
    if (paint() && !a.paused && !reduce()) raf = requestAnimationFrame(frame);
  }

  function kick() { if (!raf) raf = requestAnimationFrame(frame); }

  var events = ['play', 'seeked', 'timeupdate', 'ratechange'];
  events.forEach(function (n) { a.addEventListener(n, kick); });
  /* The active line moves by a Rust re-render: repaint as soon as it lands. */
  var mo = new MutationObserver(kick);
  mo.observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-current'] });

  function dispose() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    events.forEach(function (n) { a.removeEventListener(n, kick); });
    mo.disconnect();
    window.__owLyricFill = null;
  }
  window.__owLyricFill = dispose;
  kick();
})();
