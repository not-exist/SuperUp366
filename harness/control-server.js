// Local control server for the injected renderer hook.
// usage: node harness/control-server.js [port]
const http = require('http');
const PORT = parseInt(process.argv[2] || '7801', 10);

let seq = 0;
const queue = [];      // {id, js, match}
const waiters = [];    // {href, resolve}
const inflight = new Map();

function matchOk(match, href) {
  if (!match) return true;
  if (match[0] === '=') return (href || '') === match.slice(1);
  return (href || '').indexOf(match) >= 0;
}
function poll(href) {
  const qi = queue.findIndex((c) => matchOk(c.match, href));
  if (qi >= 0) return Promise.resolve(queue.splice(qi, 1)[0]);
  return new Promise((resolve) => {
    const entry = { href, resolve };
    const t = setTimeout(() => { const i = waiters.indexOf(entry); if (i >= 0) waiters.splice(i, 1); resolve(null); }, 20000);
    entry.resolve = (v) => { clearTimeout(t); resolve(v); };
    waiters.push(entry);
  });
}
function deliver(cmd) {
  const wi = waiters.findIndex((w) => matchOk(cmd.match, w.href));
  if (wi >= 0) { const w = waiters.splice(wi, 1)[0]; w.resolve(cmd); return true; }
  return false;
}
function body(req) {
  return new Promise((resolve) => { let b = ''; req.on('data', (c) => (b += c)); req.on('end', () => { try { resolve(JSON.parse(b || '{}')); } catch (e) { resolve({}); } }); });
}
function json(res, code, obj) {
  const s = JSON.stringify(obj);
  res.writeHead(code, { 'content-type': 'application/json', 'content-length': Buffer.byteLength(s) });
  res.end(s);
}

http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://127.0.0.1');
  if (url.pathname === '/health') return json(res, 200, { ok: true, pending: queue.length, waiters: waiters.map((w) => w.href) });

  if (url.pathname === '/poll') {
    const href = url.searchParams.get('href') || '';
    const cmd = await poll(href);
    if (!cmd) { res.writeHead(204); return res.end(); }
    return json(res, 200, { id: cmd.id, js: cmd.js });
  }
  if (url.pathname === '/result' && req.method === 'POST') {
    const b = await body(req);
    const done = inflight.get(b.id);
    if (done) { inflight.delete(b.id); done(b); }
    return json(res, 200, { ok: true });
  }
  if (url.pathname === '/cmd' && req.method === 'POST') {
    const b = await body(req);
    if (!b.js) return json(res, 400, { error: 'js required' });
    const id = ++seq;
    const cmd = { id, js: b.js, match: b.frame || '' };
    const done = new Promise((resolve) => inflight.set(id, resolve));
    if (!deliver(cmd)) queue.push(cmd);
    const timer = setTimeout(() => {
      if (inflight.has(id)) { inflight.delete(id); json(res, 200, { id, ok: false, value: 'timeout waiting for frame ' + (b.frame || '*') }); }
    }, b.timeout || 30000);
    const out = await done;
    clearTimeout(timer);
    return json(res, 200, out);
  }
  if (url.pathname === '/frames') return json(res, 200, { frames: waiters.map((w) => w.href) });
  json(res, 404, { error: 'not found' });
}).listen(PORT, '127.0.0.1', () => console.log('control server on http://127.0.0.1:' + PORT));
