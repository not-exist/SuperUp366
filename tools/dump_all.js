const fs = require('fs');
const path = require('path');
const ROOT = 'C:/Program Files (x86)/up366/client';
const OUT = 'H:/project/up366/extract';
const SKIP_EXT = new Set(['.model', '.mdl', '.fst', '.onnx', '.pak', '.dat', '.bin', '.ttf', '.wav', '.gif', '.jpg', '.jpeg', '.png', '.woff', '.woff2', '.ico', '.icns', '.exe', '.dll', '.node']);
const MAX_BYTES = 20 * 1024 * 1024;
const stats = { files: 0, bytes: 0, dirs: 0, skipped: 0, failed: [] };

function walk(srcAbs, outRel, opts) {
  let names;
  try { names = fs.readdirSync(srcAbs); } catch (e) { stats.failed.push('ls ' + outRel + ' :: ' + e.code); return; }
  stats.dirs++;
  for (const name of names) {
    if (name.endsWith('.u3enc') && opts.dedupe) continue;
    const childAbs = srcAbs + '/' + name;
    const childRel = outRel ? outRel + '/' + name : name;
    let isDir = false;
    try { fs.readdirSync(childAbs); isDir = true; } catch (e) { isDir = false; }
    if (isDir) { walk(childAbs, childRel, opts); continue; }
    const ext = path.extname(name).toLowerCase();
    if (SKIP_EXT.has(ext)) { stats.skipped++; continue; }
    if (!opts.all && !name.endsWith('.u3enc') && !opts.exts.has(ext)) { stats.skipped++; continue; }
    let buf;
    try { buf = fs.readFileSync(childAbs); } catch (e) { stats.failed.push('rd ' + childRel + ' :: ' + e.code); continue; }
    if (buf.length > MAX_BYTES) { stats.skipped++; continue; }
    let outName = childRel;
    if (outName.endsWith('.u3enc')) outName = outName.slice(0, -6);
    try {
      const out = path.join(OUT, outName);
      fs.mkdirSync(path.dirname(out), { recursive: true });
      fs.writeFileSync(out, buf);
      stats.files++; stats.bytes += buf.length;
    } catch (e) { stats.failed.push('wr ' + outName + ' :: ' + e.code); }
  }
}

walk(ROOT + '/resources/app.u3a', 'app.u3a', { all: true, dedupe: true, exts: new Set() });
walk(ROOT + '/resources/res', 'res', { all: false, dedupe: false, exts: new Set(['.html', '.js', '.css', '.json', '.txt', '.map', '.svg', '.xml']) });
walk(ROOT + '/resources/plugins/ases', 'plugins/ases', { all: false, dedupe: false, exts: new Set(['.html', '.js', '.css', '.json', '.txt']) });
console.log(JSON.stringify({ files: stats.files, bytes: stats.bytes, dirs: stats.dirs, skipped: stats.skipped, failedTotal: stats.failed.length, failed: stats.failed.slice(0, 10) }, null, 1));
