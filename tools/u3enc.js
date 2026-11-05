// .u3enc codec: AES-128-CBC, 16-byte IV prepended, PKCS#7.
// usage: node u3enc.js decrypt <in> <out> | encrypt <in> <out>
const fs = require('fs');
const crypto = require('crypto');
const KEY = Buffer.from('QJBNiBmV55PDrewyne3GsA==', 'base64');

const mode = process.argv[2], inp = process.argv[3], outp = process.argv[4];
if (!mode || !inp || !outp) { console.error('usage: node u3enc.js <decrypt|encrypt> <in> <out>'); process.exit(2); }
const data = fs.readFileSync(inp);
let out;
if (mode === 'decrypt') {
  const iv = data.subarray(0, 16);
  const d = crypto.createDecipheriv('aes-128-cbc', KEY, iv);
  out = Buffer.concat([d.update(data.subarray(16)), d.final()]);
} else if (mode === 'encrypt') {
  const iv = crypto.randomBytes(16);
  const c = crypto.createCipheriv('aes-128-cbc', KEY, iv);
  out = Buffer.concat([iv, c.update(data), c.final()]);
} else { console.error('unknown mode'); process.exit(2); }
fs.writeFileSync(outp, out);
console.log(JSON.stringify({ mode, in_bytes: data.length, out_bytes: out.length }));
