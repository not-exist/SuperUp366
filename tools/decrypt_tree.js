const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const KEY = Buffer.from('QJBNiBmV55PDrewyne3GsA==', 'base64');
const SRC = process.argv[2];
const OUT = process.argv[3];
let n = 0, bytes = 0;
function walk(d, rel) {
  for (const name of fs.readdirSync(d)) {
    const p = path.join(d, name);
    const r = rel ? rel + '/' + name : name;
    const st = fs.statSync(p);
    if (st.isDirectory()) { walk(p, r); continue; }
    if (!name.endsWith('.u3enc')) continue;
    const data = fs.readFileSync(p);
    const iv = data.subarray(0, 16);
    const dec = crypto.createDecipheriv('aes-128-cbc', KEY, iv);
    const plain = Buffer.concat([dec.update(data.subarray(16)), dec.final()]);
    const outPath = path.join(OUT, r.slice(0, -6));
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, plain);
    n++; bytes += plain.length;
  }
}
walk(SRC, '');
console.log(JSON.stringify({ files: n, bytes }));
