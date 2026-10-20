//! The small JS bridge (`window.ow`) injected into `<head>`. It only does what the DOM
//! must do synchronously: focus management, focus traps, menu arrow-key navigation.

/// Script text for `Config::with_custom_head`.
pub const BRIDGE_JS: &str = r#"
window.ow = (function () {
  var FOCUSABLE = 'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';
  var traps = {};
  function visible(e) { return !!(e.offsetWidth || e.offsetHeight || e.getClientRects().length); }
  function focusables(root) { return Array.prototype.filter.call(root.querySelectorAll(FOCUSABLE), visible); }
  /* Fixed-position a popup next to its anchor (or at x/y), flipping and clamping to the viewport. */
  function place(el, spec) {
    var pad = 8, w = el.offsetWidth, h = el.offsetHeight, vw = window.innerWidth, vh = window.innerHeight, x, y;
    var anchor = spec.anchor && document.getElementById(spec.anchor);
    if (anchor) {
      var r = anchor.getBoundingClientRect();
      x = spec.align === 'end' ? r.right - w : r.left;
      y = r.bottom + 4;
      if (y + h > vh - pad && r.top - 4 - h >= pad) y = r.top - 4 - h;
    } else {
      x = spec.x; y = spec.y;
      if (y + h > vh - pad) y = y - h;
    }
    x = Math.max(pad, Math.min(x, vw - w - pad));
    y = Math.max(pad, Math.min(y, vh - h - pad));
    el.style.left = x + 'px'; el.style.top = y + 'px'; el.style.visibility = 'visible';
  }
  return {
    focusables: focusables,
    focusId: function (id) { var e = document.getElementById(id); if (e) e.focus(); },
    /* Modal focus trap: focuses [data-autofocus] or the first control, cycles Tab, restores focus on close. */
    trapOpen: function (id) {
      var el = document.getElementById(id); if (!el) return;
      var prev = document.activeElement;
      /* Opened from a menu item: the item disappears with the menu, so return to the menu's button. */
      for (var k in traps) { if (traps[k].menu && prev && traps[k].el.contains(prev)) { prev = traps[k].prev; break; } }
      var onKey = function (e) {
        if (e.key !== 'Tab') return;
        var f = focusables(el);
        if (!f.length) { e.preventDefault(); el.focus(); return; }
        var first = f[0], last = f[f.length - 1], a = document.activeElement;
        if (e.shiftKey && (a === first || !el.contains(a))) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && (a === last || !el.contains(a))) { e.preventDefault(); first.focus(); }
      };
      document.addEventListener('keydown', onKey, true);
      traps[id] = { prev: prev, onKey: onKey };
      var body = el.querySelector('[data-dialog-body]');
      var target = el.querySelector('[data-autofocus]') || (body && focusables(body)[0]) || focusables(el)[0] || el;
      target.focus();
    },
    trapClose: function (id) {
      var t = traps[id]; if (!t) return;
      document.removeEventListener('keydown', t.onKey, true);
      delete traps[id];
      if (t.prev && document.contains(t.prev) && t.prev !== document.body) t.prev.focus();
    },
    /* Menus: focus first item, ArrowUp/Down/Home/End + typeahead, restore focus on close. */
    menuOpen: function (id, placement) {
      var el = document.getElementById(id); if (!el) return;
      place(el, placement);
      var prev = document.activeElement;
      var items = function () { return Array.prototype.filter.call(el.querySelectorAll('[role^="menuitem"]'), function (i) { return !i.disabled; }); };
      var onKey = function (e) {
        var list = items(), i = list.indexOf(document.activeElement), n = list.length; if (!n) return;
        var next = -1;
        if (e.key === 'ArrowDown') next = (i + 1) % n;
        else if (e.key === 'ArrowUp') next = (i - 1 + n) % n;
        else if (e.key === 'Home') next = 0;
        else if (e.key === 'End') next = n - 1;
        else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          var c = e.key.toLowerCase();
          for (var k = 1; k <= n; k++) { var cand = list[(Math.max(i, -1) + k) % n]; if (cand.textContent.trim().toLowerCase().indexOf(c) === 0) { next = list.indexOf(cand); break; } }
        }
        if (next >= 0) { e.preventDefault(); e.stopPropagation(); list[next].focus(); }
      };
      el.addEventListener('keydown', onKey);
      traps[id] = { prev: prev, el: el, onKey: onKey, menu: true };
      var l = items(); if (l.length) l[0].focus(); else el.focus();
    },
    menuClose: function (id) {
      var t = traps[id]; if (!t) return;
      t.el.removeEventListener('keydown', t.onKey);
      delete traps[id];
      if (t.prev && document.contains(t.prev) && t.prev !== document.body) t.prev.focus();
    },
    focusSearch: function () {
      var e = document.getElementById('ow-global-search');
      if (e) { e.focus(); if (e.select) e.select(); }
    }
  };
})();
"#;

/// Document-level shortcut listener. Sends action names back to Rust via `dioxus.send`.
/// It never calls `blur()`, and leaves keys alone when the focused control owns them.
pub const SHORTCUT_JS: &str = r#"
(function () {
  if (window.__owKeys) { window.__owKeys(); }
  var typing = function (t) {
    if (!t || !t.tagName) return false;
    var n = t.tagName.toLowerCase();
    return n === 'input' || n === 'textarea' || n === 'select' || t.isContentEditable;
  };
  var activatable = function (t) {
    if (!t || !t.closest) return false;
    return !!t.closest('button,a[href],summary,[role="button"],[role="tab"],[role="switch"],[role^="menuitem"],[role="checkbox"],[role="radio"],[role="option"]');
  };
  var ownsArrows = function (t) {
    if (!t || !t.closest) return false;
    return !!t.closest('[role="slider"],[role="tab"],[role="radio"],[role^="menuitem"],[role="option"],[role="listbox"],[role="tablist"],[role="radiogroup"]');
  };
  var onKey = function (e) {
    if (e.defaultPrevented || e.isComposing) return;
    var t = e.target, k = e.key, ctrl = e.ctrlKey;
    if (e.altKey || e.metaKey) return;
    var overlay = !!document.querySelector('[data-ow-overlay]');
    var send = function (name, prevent) { if (prevent) e.preventDefault(); dioxus.send(name); };
    if (ctrl && (k === 'k' || k === 'K')) return send('palette', true);
    if (overlay) return;
    if (typing(t)) return;
    if (k === 'Escape') return send('escape', false);
    if (k === '/' && !ctrl) { e.preventDefault(); window.ow.focusSearch(); dioxus.send('focus_search'); return; }
    if (ctrl && (k === 'l' || k === 'L')) return send('like', true);
    if (ctrl && k === 'ArrowLeft') return send('previous', true);
    if (ctrl && k === 'ArrowRight') return send('next', true);
    if (ctrl) return;
    if (k === ' ' || k === 'Spacebar') { if (activatable(t)) return; return send('play_pause', true); }
    if (k === 'ArrowLeft' || k === 'ArrowRight' || k === 'ArrowUp' || k === 'ArrowDown') {
      if (ownsArrows(t)) return;
      var map = { ArrowLeft: 'seek_back', ArrowRight: 'seek_forward', ArrowUp: 'volume_up', ArrowDown: 'volume_down' };
      return send(map[k], true);
    }
    var low = k.length === 1 ? k.toLowerCase() : '';
    if (low === 'm') return send('mute', false);
    if (low === 'q') return send('queue', false);
    if (low === 'l') return send('lyrics', false);
    if (low === 'f') return send('fullscreen', false);
    if (low === 'v') return send('visualizer', false);
    if (low === 'p') return send('mini', false);
  };
  document.addEventListener('keydown', onKey);
  window.__owKeys = function () { document.removeEventListener('keydown', onKey); };
})();
"#;
