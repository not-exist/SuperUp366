// CLI for driving the injected renderer.
// usage: node harness/u3.js <command> [args]
const http = require('http');

const PORT = parseInt(process.env.U3_PORT || '7801', 10);

function post(path, obj, timeout) {
  return new Promise((resolve, reject) => {
    const data = Buffer.from(JSON.stringify(obj));
    const req = http.request({ host: '127.0.0.1', port: PORT, path, method: 'POST', headers: { 'content-type': 'application/json', 'content-length': data.length } }, (res) => {
      let b = '';
      res.on('data', (c) => (b += c));
      res.on('end', () => { try { resolve(JSON.parse(b)); } catch (e) { resolve(b); } });
    });
    req.on('error', reject);
    req.setTimeout(timeout || 35000, () => { req.destroy(new Error('timeout')); });
    req.end(data);
  });
}

async function evalJs(js) {
  const r = await post('/cmd', { js, frame: process.env.U3_FRAME || '' });
  if (!r.ok) throw new Error(r.value || 'eval failed');
  return r.value;
}

const q = (s) => JSON.stringify(s);

(async () => {
  const [cmd, ...rest] = process.argv.slice(2);
  try {
    let out;
    switch (cmd) {
      case 'ping': out = await evalJs('__U3DEV__.ping()'); break;
      case 'eval': out = await evalJs(rest.join(' ')); break;
      case 'dom': out = await evalJs('__U3DEV__.dom(' + (rest[0] ? q(rest[0]) : 'null') + ')'); break;
      case 'click': out = await evalJs('__U3DEV__.click(' + q(rest[0]) + ')'); break;
      case 'type': out = await evalJs('__U3DEV__.type(' + q(rest[0]) + ',' + q(rest[1] || '') + ',' + q(rest[2] !== 'false') + ')'); break;
      case 'wait': out = await evalJs('__U3DEV__.waitFor(' + q(rest[0]) + ',' + (rest[1] || 10000) + ')'); break;
      case 'css': out = await evalJs('__U3DEV__.css(' + q(rest.join(' ')) + ')'); break;
      default:
        console.log('commands: ping | eval <js> | dom [sel] | click <sel> | type <sel> <text> [enter=false] | wait <sel> [ms] | css <text>');
        process.exit(cmd ? 2 : 0);
    }
    console.log(typeof out === 'string' ? out : JSON.stringify(out, null, 1));
  } catch (e) {
    console.error('ERR', e.message);
    process.exit(1);
  }
})();
