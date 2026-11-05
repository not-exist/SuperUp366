// Injected renderer hook. Provides request logging, mocking, and a control
// channel to harness/control-server.js. No Node access in this context.
(function () {
  var CTRL = 'http://127.0.0.1:7801';
  var VERSION = 2;
  window.__U3DEV__ = { version: VERSION };

  function log() {
    try { console.log.apply(console, ['[u3dev]'].concat([].slice.call(arguments))); } catch (e) {}
  }

  // ---- request observation -------------------------------------------------
  var origFetch = window.fetch;
  if (origFetch) {
    window.fetch = function (input, init) {
      var url = typeof input === 'string' ? input : (input && input.url);
      if (!url || url.indexOf(CTRL) !== 0) log('fetch', (init && init.method) || 'GET', url);
      return origFetch.apply(this, arguments);
    };
  }
  var OrigXHR = window.XMLHttpRequest;
  if (OrigXHR) {
    var xopen = OrigXHR.prototype.open, xsend = OrigXHR.prototype.send;
    OrigXHR.prototype.open = function (m, u) { this.__u3 = { m: m, u: u }; return xopen.apply(this, arguments); };
    OrigXHR.prototype.send = function () { if (this.__u3) log('xhr', this.__u3.m, this.__u3.u); return xsend.apply(this, arguments); };
  }
  var OrigWS = window.WebSocket;
  if (OrigWS) {
    var WrappedWS = function (url, p) { log('ws', url); return p ? new OrigWS(url, p) : new OrigWS(url); };
    WrappedWS.prototype = OrigWS.prototype;
    window.WebSocket = WrappedWS;
  }

  // ---- helpers -------------------------------------------------------------
  window.__U3DEV__.css = function (text) {
    var el = document.createElement('style');
    el.textContent = text;
    document.head.appendChild(el);
    return 'ok';
  };
  window.__U3DEV__.dom = function (sel) {
    var el = sel ? document.querySelector(sel) : document.body;
    return el ? el.outerHTML.slice(0, 200000) : null;
  };
  window.__U3DEV__.click = function (sel) {
    var el = document.querySelector(sel);
    if (!el) return 'missing: ' + sel;
    el.click();
    return 'clicked';
  };
  window.__U3DEV__.type = function (sel, text, enter) {
    var el = document.querySelector(sel);
    if (!el) return 'missing: ' + sel;
    el.focus();
    var setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
    setter.call(el, text);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    if (enter) {
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', keyCode: 13, bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', keyCode: 13, bubbles: true }));
      if (el.form) el.form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }
    return 'typed';
  };
  window.__U3DEV__.waitFor = function (sel, ms) {
    ms = ms || 10000;
    var start = Date.now();
    return new Promise(function (resolve) {
      (function tick() {
        if (document.querySelector(sel)) return resolve('found');
        if (Date.now() - start > ms) return resolve('timeout: ' + sel);
        setTimeout(tick, 200);
      })();
    });
  };
  window.__U3DEV__.ping = function () { return { href: location.href, title: document.title, ua: navigator.userAgent }; };

  // ---- bridge call tracing -------------------------------------------------
  var calls = [];
  window.__U3DEV__.logs = calls;
  function trace(obj, name) {
    try {
      if (!obj || typeof obj[name] !== 'function') return;
      var orig = obj[name];
      obj[name] = function () {
        var args = [].slice.call(arguments);
        var rec = { t: Date.now(), fn: name, args: args.map(function (a) {
          try { return typeof a === 'object' ? JSON.parse(JSON.stringify(a)) : a; } catch (e) { return String(a); }
        }) };
        calls.push(rec);
        if (calls.length > 200) calls.shift();
        log('bridge', name, rec.args);
        try { return orig.apply(this, arguments); } catch (e) { rec.err = String(e); throw e; }
      };
    } catch (e) { log('trace failed', name, String(e)); }
  }
  ['openUrl', 'getConfigUrl', 'showMainView', 'openWindow', 'showDialog', 'sendMessage'].forEach(function (n) {
    trace(window.CurrentWindow, n);
  });
  ['getUserInfo', 'getToken', 'login', 'logout'].forEach(function (n) { trace(window.Login, n); });

  // ---- control channel -----------------------------------------------------
  var alive = false;
  function post(payload) {
    if (!origFetch) return;
    try {
      origFetch.call(window, CTRL + '/result', { method: 'POST', body: JSON.stringify(payload), headers: { 'content-type': 'application/json' } });
    } catch (e) {}
  }
  function safe(v) {
    try { return JSON.parse(JSON.stringify(v)); } catch (e) { return String(v); }
  }
  function schedule(ms) { setTimeout(poll, ms); }
  function poll() {
    if (!origFetch) return;
    origFetch.call(window, CTRL + '/poll?href=' + encodeURIComponent(location.href), { cache: 'no-store' })
      .then(function (r) { return r.status === 204 ? null : r.json(); })
      .then(function (cmd) {
        alive = true;
        if (!cmd) return schedule(150);
        new Promise(function (res) { res(eval(cmd.js)); })
          .then(function (v) { post({ id: cmd.id, ok: true, value: safe(v) }); },
                function (e) { post({ id: cmd.id, ok: false, value: String((e && e.stack) || e) }); })
          .then(function () { schedule(30); });
      })
      .catch(function () { alive = false; schedule(2000); });
  }
  schedule(300);
  Object.defineProperty(window.__U3DEV__, 'connected', { get: function () { return alive; } });
})();
