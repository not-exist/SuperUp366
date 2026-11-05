const fs = require('fs');
const A = 'C:/Program Files (x86)/up366/client/resources/app.u3a';
const out = [];
function walk(rel, depth) {
  const abs = rel ? A + '/' + rel : A;
  let names;
  try { names = fs.readdirSync(abs); } catch (e) { out.push({ rel: rel, err: 'ls ' + e.code, depth: depth }); return; }
  out.push({ rel: rel || '.', kind: 'dir', n: names.length, depth: depth, names: names.slice(0, 30) });
  for (const n of names) {
    const childRel = rel ? rel + '/' + n : n;
    let isDir = false;
    try { fs.readdirSync(abs + '/' + n); isDir = true; } catch (e) { isDir = false; }
    if (isDir) { walk(childRel, depth + 1); continue; }
    try { const b = fs.readFileSync(abs + '/' + n); out.push({ rel: childRel, size: b.length, depth: depth + 1, head: b.slice(0, 24).toString('utf8').replace(/[^\x20-\x7e]/g, '.') }); }
    catch (e) { out.push({ rel: childRel, err: 'rd ' + e.code, depth: depth + 1 }); }
  }
}
walk('', 0);
console.log(JSON.stringify(out, null, 0));
