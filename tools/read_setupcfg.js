// 本地成绩库读写（SQLCipher）。
//
// 关键事实（recon/readaloud-complete.md §3.3）：
//   - 库文件：登录用户的数据在 C:/Up366StudentFiles/user/<username>/setup.cfg；
//     未登录时的公共库是 C:/Up366StudentFiles/setup.cfg。
//   - key = "{3E2A243F-7B8B-45C8-9E7D-885A891820A7}" + username（per-user 库要拼用户名）。
//   - 表名全小写：taskstuded / teststuded / recordfiles / commonupload / …
//   - libsqlite.node 是 ia32/win32 原生模块，必须 32 位运行时（up366.exe + ELECTRON_RUN_AS_NODE=1）。
//
// 用法：
//   ELECTRON_RUN_AS_NODE=1 "<up366.exe>" tools/read_setupcfg.js dump --user <username>
//   ELECTRON_RUN_AS_NODE=1 "<up366.exe>" tools/read_setupcfg.js insert-records --user <username> --json rows.json
//   ELECTRON_RUN_AS_NODE=1 "<up366.exe>" tools/read_setupcfg.js insert-score --user <username> --json row.json
//   ELECTRON_RUN_AS_NODE=1 "<up366.exe>" tools/read_setupcfg.js backup --user <username>
const path = require('path');
const fs = require('fs');

const SQLITE = 'C:/Program Files (x86)/up366/client/lib/ia32/win32/libsqlite.node';
const KEY = '{3E2A243F-7B8B-45C8-9E7D-885A891820A7}';
const SAVE_DIR = 'C:/Up366StudentFiles';

const a = require(SQLITE);

function dbPath(user) {
  return user ? path.join(SAVE_DIR, 'user', user, 'setup.cfg') : path.join(SAVE_DIR, 'setup.cfg');
}

function openDb(user, explicitPath) {
  const f = explicitPath || dbPath(user);
  const db = new a.Database(f, 'rwc', false, false, false, 0, true, true);
  db.exec('PRAGMA key = "' + KEY + (user || '') + '"', {}, true);
  return { db: db, file: f };
}

// 表名（小写）与列清单，取自客户端表结构对象 main/index.js@3526046 附近
const COLUMNS = {
  recordfiles: ['id', 'pageId', 'taskId', 'recordId', 'bookId', 'recordFile', 'objectName',
    'srcType', 'studyTime', 'failed', 'uploaded', 'fileSize', 'msg'],
  taskstuded: ['id', 'taskId', 'uid', 'bookId', 'chapterId', 'taskNo', 'score', 'percent',
    'studyDate', 'seconds', 'result', 'srcType', 'batchTag', 'uploaded', 'msg', 'studyTaskId',
    'courseId', 'extendParams', 'qstJson', 'failed', 'killStudyTask', 'submitType'],
};

function insertRow(db, table, obj) {
  const cols = COLUMNS[table];
  if (!cols) throw new Error('未知表 ' + table);
  const use = cols.filter(c => obj[c] !== undefined);
  if (!use.length) throw new Error('没有可写字段');
  const sql = 'insert into ' + table + '(' + use.join(',') + ') values(' +
    use.map(c => ':' + c).join(',') + ')';
  // 用具名参数绑定：prepare 的 run 接受对象，键不带冒号
  const stmt = db.prepare(sql, {}, true);
  const params = {};
  for (const c of use) params[c] = obj[c];
  stmt.run(params);
}
function dump(db) {
  const tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table'", {}, true).all()
    .map(r => r.name).filter(n => !/^sqlite_/.test(n));
  const out = {};
  for (const t of tables) {
    try {
      out[t] = db.prepare('SELECT * FROM ' + t, {}, true).all();
    } catch (e) {
      out[t] = 'ERR ' + e.message;
    }
  }
  return out;
}

function parseArgs(argv) {
  const cmd = argv[0];
  const opts = {};
  for (let i = 1; i < argv.length; i++) {
    if (argv[i].startsWith('--')) opts[argv[i].slice(2)] = argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : true;
  }
  return { cmd: cmd, opts: opts };
}

// argv 布局有两种：
//   -e "$(cat x.js)" dump --user u   → argv = [exe, 'dump', '--user', 'u']
//   （直接给脚本路径在本机不可用：厂商 fs 补丁会改写 .js 的解析）
//   node x.js dump --user u          → argv = [node, x.js, 'dump', '--user', 'u']
function scriptArgs(argv) {
  const rest = argv.slice(1);
  if (rest.length && /\.js$/i.test(rest[0])) rest.shift();
  return rest;
}

const { cmd, opts } = parseArgs(scriptArgs(process.argv));

if (cmd === 'dump') {
  const { db, file } = openDb(opts.user || '', opts.db);
  const data = dump(db);
  console.log('FILE ' + file);
  for (const t of Object.keys(data)) {
    const rows = data[t];
    console.log('== ' + t + ' rows=' + (Array.isArray(rows) ? rows.length : rows));
    if (Array.isArray(rows)) for (const r of rows.slice(0, 10)) console.log('   ' + JSON.stringify(r).slice(0, 1500));
  }
  try { db.close(); } catch (x) {}
} else if (cmd === 'insert-records' || cmd === 'insert-score') {
  const table = cmd === 'insert-records' ? 'recordfiles' : 'taskstuded';
  const { db, file } = openDb(opts.user || '', opts.db);
  const rows = JSON.parse(fs.readFileSync(opts.json, 'utf8'));
  const list = Array.isArray(rows) ? rows : [rows];
  // 幂等：先按 taskId 清掉旧行
  const taskIds = Array.from(new Set(list.map(r => r.taskId).filter(Boolean)));
  for (const tid of taskIds) {
    db.prepare('delete from ' + table + ' where taskId=:taskId', {}, true).run({ taskId: tid });
  }
  for (const r of list) insertRow(db, table, r);
  const back = db.prepare('SELECT * FROM ' + table, {}, true).all();
  console.log('FILE ' + file);
  console.log('inserted ' + list.length + ' rows into ' + table + '; now rows=' + back.length);
  for (const r of back) console.log('   ' + JSON.stringify(r).slice(0, 600));
  try { db.close(); } catch (x) {}
} else if (cmd === 'backup') {
  const { file } = openDb(opts.user || '', opts.db);
  const dst = (opts.out || ('work/setup.cfg.bak.' + Date.now()));
  fs.mkdirSync(path.dirname(dst), { recursive: true });
  fs.copyFileSync(file, dst);
  console.log('backup ' + file + ' -> ' + dst);
} else if (cmd === 'userinfo') {
  // 当前用户的 uid / 学校 id / utype / username —— 提交与上传都要用
  const { db, file } = openDb(opts.user || '', opts.db);
  const row = db.prepare('SELECT value FROM cachedata WHERE key=:key', {}, true).get({ key: 'USER_INFO' });
  if (!row) {
    console.log('USER_INFO 不存在（该库未登录过？）');
  } else {
    let u = {};
    try { u = JSON.parse(row.value); } catch (e) { console.log('USER_INFO 解析失败: ' + e.message); }
    console.log(JSON.stringify({
      file: file,
      uid: u.uid,
      username: u.username,
      schoolId: u.organId,
      schoolName: u.organName,
      type: u.utype,
      uuid: u.uuid,
      realname: u.realname,
    }, null, 2));
  }
  try { db.close(); } catch (x) {}
} else {
  console.log('用法: read_setupcfg.js <dump|insert-records|insert-score|backup|userinfo> [--user <username>] [--json <file>]');
  process.exit(1);
}
