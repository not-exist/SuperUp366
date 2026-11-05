const fs = require('fs');
const path = require('path');
const A = 'C:/Program Files (x86)/up366/client/resources/app.u3a';
const OUT = 'H:/project/up366/extract/app.u3a';
const stats = { dirs: 0, files: 0, bytes: 0, failed: [] };
function walk(rel) {
  const abs = rel ? A + '/' + rel : A;
  let names;
  try { names = fs.readdirSync(abs); } catch (e) { stats.failed.push(rel + ' ls ' + e.code); return; }
  stats.dirs++;
  for (const n of names) {
    const childRel = rel ? rel + '/' + n : n;
    let isDir = false;
    try { fs.readdirSync(abs + '/' + n); isDir = true; } catch (e) { isDir = false; }
    if (isDir) { walk(childRel); continue; }
    let buf;
    try { buf = fs.readFileSync(abs + '/' + n); } catch (e) { stats.failed.push(childRel + ' rd ' + e.code); continue; }
    let outName = childRel.endsWith('.u3enc') ? childRel.slice(0, -6) : childRel;
    const out = path.join(OUT, outName);
    try {
      fs.mkdirSync(path.dirname(out) === OUT ? OUT : path.dirname(out), { recursive: true });
      fs.writeFileSync(out, buf);
      stats.files++; stats.bytes += buf.length;
    } catch (e) { stats.failed.push(outName + ' wr ' + e.code + ' ' + String(e.message).slice(0, 50)); }
  }
}
walk('');
console.log(JSON.stringify(stats, null, 1));
