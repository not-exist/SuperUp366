# 前端控制 harness

不改动安装目录，只在 `work/client` 副本上注入页面脚本。

## 用法

```powershell
# 1. 首次准备（复制安装目录 + 加密注入 index.html + 投放 _dev/hook.js）
powershell -NoProfile -ExecutionPolicy Bypass -File harness\dev-prepare.ps1

# 2. 后台启动（最小化、独立 user-data-dir、12 秒后自动关闭）
powershell -NoProfile -ExecutionPolicy Bypass -File harness\dev-launch.ps1

# 3. 常驻（不自动关闭）
powershell -NoProfile -ExecutionPolicy Bypass -File harness\dev-launch.ps1 -Keep
```

## 改哪里

| 文件 | 作用 |
|---|---|
| `harness/payload/index.html` | 被加密后覆盖 `work/client/resources/res/index.html.u3enc` |
| `harness/payload/_dev/hook.js` | 运行时注入的脚本，直接复制到 `work/client/resources/res/_dev/hook.js` |

`hook.js` 改动**不需要重新加密**：直接改 `work/client/resources/res/_dev/hook.js` 再启动即可；
要恢复成 payload 版本就重跑 `dev-prepare.ps1`。

## hook.js 提供的接口

- `window.__U3DEV__.mock(urlSubstr, handler)` — 拦截 fetch，返回自定义 JSON
- `window.__U3DEV__.css(text)` — 注入样式
- 默认已包装 `fetch` / `XMLHttpRequest` / `WebSocket`，所有请求打到 `console`（前缀 `[u3dev]`）

## 原理

1. `.u3enc` = 16 字节 IV + AES-128-CBC + PKCS#7，密钥硬编码在 `up366.exe`（`tools/u3enc.js`）。
   因此可以**重新加密**任意前端资源。
2. 实测：替换 `resources/res/index.html.u3enc` 后客户端正常加载，无完整性校验拦截。
3. 渲染窗口 `contextIsolation:false`、`webSecurity:false`、`webviewTag:true`，
   但 **无 nodeIntegration**（页面里 `require` 未定义），所以 hook 拿的是完整 DOM/网络能力，不是 Node 能力。

## 已知限制

- 只对 `resources/res/` 下的页面生效（Vite 入口 + static 老页面），
  `app.u3a` 归档内的 `dist/res/loading` 不在其中。
- 更新器可能覆盖 `work/client`；重跑 `dev-prepare.ps1 -Resync` 即可恢复。

## E2E 控制通道（无需 CDP）

客户端禁掉了 `--remote-debugging-port` / `--inspect` / `NODE_OPTIONS`，
所以用页面内 hook + 本地 HTTP 轮询做控制通道。

```powershell
# 终端 A：控制服务（必须先起）
node harness\control-server.js 7801

# 终端 B：启动客户端（-Offscreen 把窗口挪到屏幕外，保证 Chromium 正常渲染但不打扰）
powershell -NoProfile -ExecutionPolicy Bypass -File harness\dev-launch.ps1 -Offscreen -Keep
```

```bash
node harness/u3.js ping                 # 当前窗口 URL / title / UA
curl -s 127.0.0.1:7801/frames           # 列出所有已连上的窗口/iframe

# 客户端是多窗口 + 多 frame，必须用 U3_FRAME 指定（子串匹配 location.href）
export MSYS_NO_PATHCONV=1               # Git Bash 会把 "#/login" 里的 / 变成路径
U3_FRAME="#/login"     node harness/u3.js eval "document.title"
U3_FRAME="login.html"  node harness/u3.js click ".login-btn"
U3_FRAME="#/menu"      node harness/u3.js eval "document.body.textContent.slice(0,200)"
```

命令：`ping` / `eval <js>` / `dom [sel]` / `click <sel>` / `type <sel> <text> [enter]` / `wait <sel> [ms]` / `css <text>`

### 实测流程（2026-11-01）

1. 启动后 frame 列表：`local://res/index.html#/menu`、`#/dialog`、`#/toolbar`、`#/login`、`local://res/login.html`。
2. 登录表单在 `#/login` 窗口的 iframe（`login.html`）里，账号密码由客户端自动填充。
3. `U3_FRAME="login.html" node harness/u3.js click ".login-btn"` → 登录成功。
4. 登录后窗口集合变为 `#/`、`#/menu`、`#/dialog`、`#/toolbar`；`#/` 显示「我的教学班 / 错题&生词 / 消息」等主体内容。
5. 窗口最小化时 `document.visibilityState === "hidden"`，`innerText` 为空、部分组件不渲染；
   `-Offscreen` 或 `harness/offscreen.ps1` 把窗口移到屏幕外并 ShowWindow，`visibilityState` 变为 `visible`，页面完整渲染。

### 注意

- 控制通道是页面内 `fetch` 轮询 `127.0.0.1:7801`，依赖 `corsEnabled` + `webSecurity:false`，客户端升级后可能失效。
- 多 frame 同时轮询，服务器按 `href` 子串派发命令；不指定 `U3_FRAME` 时命令会落到任意一个连上的 frame。
- 页面没有 Node（`require is not defined`），只有 DOM + 网络。需要主进程能力见 `recon/security.md` 的插件目录加载路径。
