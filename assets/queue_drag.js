/* Pointer-based drag reorder for the queue list (HTML5 drag and drop is unreliable inside the
 * desktop webview). Rows are [data-uid] inside [data-qlist]; the grip is [data-qhandle].
 * Dragging only moves CSS transforms (no re-render per frame); on release one message
 * {uid, before} goes to Rust ("before" = uid of the row it lands in front of, 0 = at the end).
 */
(function () {
  if (window.__owQueueDrag) window.__owQueueDrag();
  var st = null;
  var reduce = function () {
    return document.documentElement.hasAttribute('data-low-power') ||
      (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
  };

  function reset() {
    if (!st) return;
    st.rows.forEach(function (r) {
      r.style.transform = ''; r.style.transition = ''; r.style.zIndex = ''; r.style.position = '';
      r.style.boxShadow = ''; r.style.pointerEvents = '';
    });
    st = null;
  }

  function onDown(e) {
    if (e.button !== 0) return;
    var h = e.target.closest && e.target.closest('[data-qhandle]');
    if (!h) return;
    var row = h.closest('[data-uid]');
    var list = row && row.closest('[data-qlist]');
    if (!row || !list) return;
    e.preventDefault();
    var rows = Array.prototype.slice.call(list.querySelectorAll('[data-uid]'));
    var tops = rows.map(function (r) { var b = r.getBoundingClientRect(); return b.top + b.height / 2; });
    st = {
      row: row, rows: rows, centers: tops, idx: rows.indexOf(row), target: rows.indexOf(row),
      y0: e.clientY, h: row.getBoundingClientRect().height,
      scroller: list.closest('[data-qscroll]') || list, scroll0: 0
    };
    st.scroll0 = st.scroller.scrollTop;
    row.style.position = 'relative'; row.style.zIndex = '5';
    row.style.boxShadow = 'var(--ow-shadow-pop)';
    row.style.transition = 'none';
    rows.forEach(function (r) { if (r !== row && !reduce()) r.style.transition = 'transform 120ms ease-out'; });
    document.addEventListener('pointermove', onMove);
    document.addEventListener('pointerup', onUp);
    document.addEventListener('pointercancel', onCancel);
  }

  function onMove(e) {
    if (!st) return;
    var sr = st.scroller.getBoundingClientRect();
    if (e.clientY < sr.top + 32) st.scroller.scrollTop -= 12;
    else if (e.clientY > sr.bottom - 32) st.scroller.scrollTop += 12;
    var dy = e.clientY - st.y0 + (st.scroller.scrollTop - st.scroll0);
    st.row.style.transform = 'translateY(' + dy + 'px)';
    var c = st.centers[st.idx] + dy, t = 0;
    st.centers.forEach(function (m, i) { if (i !== st.idx && m < c) t += 1; });
    st.target = t;
    st.rows.forEach(function (r, i) {
      if (i === st.idx) return;
      var shift = 0;
      if (st.idx < i && i <= t) shift = -st.h;
      else if (t <= i && i < st.idx) shift = st.h;
      r.style.transform = shift ? 'translateY(' + shift + 'px)' : '';
    });
  }

  function cleanup() {
    document.removeEventListener('pointermove', onMove);
    document.removeEventListener('pointerup', onUp);
    document.removeEventListener('pointercancel', onCancel);
  }
  function onCancel() { cleanup(); reset(); }
  function onUp() {
    cleanup();
    if (!st) return;
    var uid = Number(st.row.getAttribute('data-uid'));
    var idx = st.idx, t = st.target, rows = st.rows;
    reset();
    if (t === idx) return;
    var order = rows.slice(); var moved = order.splice(idx, 1)[0]; order.splice(t, 0, moved);
    var next = order[t + 1];
    dioxus.send({ uid: uid, before: next ? Number(next.getAttribute('data-uid')) : 0 });
  }

  document.addEventListener('pointerdown', onDown);
  window.__owQueueDrag = function () { document.removeEventListener('pointerdown', onDown); cleanup(); reset(); };
})();
