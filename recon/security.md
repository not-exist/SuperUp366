# 天学网学生客户端 (up366) 客户端安全评估

- 评估对象：`app.u3a`（Electron 22.3.27 / Node 16.17.1 / Chromium 108）已解密产物
- 输入：`extract/app.u3a/dist/main/index.js`、`extract/app.u3a/dist/main/main-window.js`、`extract/app.u3a/dist/preload/*.js`、`extract/plugins/ases/index.js`
- 方法：纯静态阅读（grep + 偏移切片），未启动客户端、未联网、未执行任何样本代码
- 引用格式：`文件@字节偏移`（bundle 为单行压缩，偏移可直接 `python -c` 切片复核）

## 0. 结论摘要

| # | 级别 | 标题 |
|---|---|---|
| 1 | 高危 | 15 个桥接对象 / 258 个 RPC 方法全量暴露，含任意文件读、任意文件写、任意路径复制/删除、任意 `shell.openExternal` |
| 2 | 高危 | 全部窗口 `contextIsolation:false` + `webSecurity:false` + `webviewTag` + 全局 `--no-sandbox`，preload 注入所有子框架，远程页面即获得上述全部能力 |
| 3 | 高危 | `setWindowOpenHandler` 无条件 allow + `will-attach-webview` 强制覆盖，任意远程 URL 被装进特权窗口/webview |
| 4 | 高危 | `local://` 协议路径穿越与 `local://path/<绝对路径>` 任意文件读取，且带 `bypassCSP`/`corsEnabled` |
| 5 | 高危 | 更新链：包 URL 的 http→https「升级」是空操作、仅校验响应体自带的 MD5、客户端无降级校验；更新器以管理员权限运行 |
| 6 | 高危 | 插件目录（`%saveDir%\plugins`，用户可写）无签名校验地 `require`/`vm.runInThisContext`，可持久化主进程代码执行 |
| 7 | 中危 | `redirect://` 协议按调用方传入的 `service` 返回 CAS ticket，无来源校验且 `corsEnabled` |
| 8 | 中危 | `request`/`httpRequest` 为渲染进程提供主进程 HTTP 代理（SSRF / 任意头 / 返回体回传） |
| 9 | 中危 | `checkResourceRootDir` 仅做字符串前缀比较，`rename` 只校验目标路径，`fileRm` 仅要求路径含 `up366` |
| 10 | 低危 | 主进程 `executeJavaScript` 拼接未转义字符串（`showLoading({msg})` 模板串、loading 窗口错误信息只转义了双引号） |
| 11 | 低危 | Linux 分支用 shell 字符串执行 `bash /tmp/up366update`（`process.argv` 直接拼接）；`chmod +x "${path}"` |
| 12 | 低危 | `resources/app.json` 的 `ignoreCertificate` 可全局关闭 TLS 校验（`NODE_TLS_REJECT_UNAUTHORIZED=0`） |
| 13 | 低危 | 第三方 `systeminformation` 大量 `exec`/`execSync` 被编入主进程，常驻攻击面 |
| 14 | 高危 | `Login.getUserInfo()`/`getToken()` 是 RPC 方法，任意渲染进程可直读**明文密码**与 TGT（不必落盘取证） |
| 15 | 中危 | 「记住密码」把明文密码写入 SQLCipher 库，PRAGMA key 硬编码在 bundle 内 → 落盘等同明文 |
| 16 | 中危 | 直播子进程把 `up366Password`（明文）/token/UserSig 以 base64url 塞进 `up366course://?…` 命令行，本机任意进程可读 |
| 17 | 中危 | `#/dialog` 用 `innerHTML` 渲染主进程下发的 opts.title/content，无 CSP + 桥接 → 文案注入即 RCE（第 9 节） |
| 18 | 中危 | 离线播放器 `loadScript(resPath+"page1.js")` + `eval('('+responseText+')')`，resPath 来自无签名的书籍包 → 渲染层代码执行 |

高危 7 项，中危 7 项，低危 4 项。

---

## 1. BrowserWindow 配置

### 1.1 证据

**启动参数（main/index.js@3522755、@3522987、@3523604）**

```js
app.commandLine.appendSwitch("disable-site-isolation-trials")
appExtParams?.ignoreCertificate && (process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0",
    app.commandLine.appendSwitch("ignore-certificate-errors", "true"))
app.commandLine.appendSwitch("high-dpi-support", "1")
app.commandLine.appendSwitch("force-device-scale-factor", "1")
process.platform !== "darwin" && app.commandLine.appendSwitch("no-sandbox")
```

**主窗口基类 `rn`（main-window.js@4291033，默认值 @4291704）**

```js
webPreferences: {
  preload: path.join(__dirname, "../preload/index.js"),
  contextIsolation: !1,
  webSecurity: !1,
  disableHtmlFullscreenWindowResize: !0,
  nodeIntegrationInSubFrames: !0,
  nodeIntegrationInWorker: !0,
  devTools: !!zA().devtool || !1,
  additionalArguments: [`--log=${zA().log || "info"}`],
  webviewTag: !0
}
```

未显式设置 `nodeIntegration`、`sandbox`、`enableRemoteModule`、`allowRunningInsecureContent`。这意味着：`nodeIntegration` 取 Electron 22 默认 `false`，但 `--no-sandbox` 已在进程级关闭 OS 沙箱，`contextIsolation:false` 让 preload 与页面共享同一个 JS 上下文——preload 挂在 `window` 上的对象即页面的对象。

**所有派生窗口（同一组不安全默认值，仅替换 preload）**

| 窗口 / 视图 | 证据 | 差异 |
|---|---|---|
| 主窗口 `rn` | @4291033 / @4291704 | 基类，如上 |
| 对话框 `mM` | @4302667 `super("index.html#/dialog", …)` | 继承全部默认值 |
| 菜单窗口 `menuWindow` | @4318917 附近 `new rn("index.html#/menu",{transparent:!0,width:0,height:0,...})` | 透明、置顶、`close` 被 `preventDefault` |
| 工具条/浮窗 `ZC` | @4307934 | `transparent:!0,resizable:!1,show:!1` |
| 词典窗 `yM` | @4307166 `super(t,{…,webPreferences:{preload:…/preload/dict.js}})` | 仅换 preload，其余默认值由 `defaultsDeep` 补全为不安全值 |
| PDF 导出窗 `kv` | @4316166 | 隐藏窗口，`backgroundThrottling:!1` |
| 内容视图 `qc extends BrowserView` | @4296675 | `contextIsolation:!1, webSecurity:!1, nodeIntegrationInWorker:!0, nodeIntegrationInSubFrames:!0` |
| 教材/答题视图 | @4301125 `BM`(flipbook-catalog)、`iv`(flipbook-study)、`NQ`(jssdk-2023)、`$Q`(jssdk-2025) | 仅替换 preload |
| loading 窗口 | main/index.js@3523900 | **唯一**未传 `webPreferences` 的窗口（默认 contextIsolation=true / sandbox 生效但被 `--no-sandbox` 抵消）；只 `loadFile("../res/loading/index.html")` |
| micTest.html / network.html / xotFullScreen | @4205208 `showView("device",{url:Lp("micTest.html")})`；@4242804 `full_screen` → `renderer/xotFullScreen/index.html` | 复用 `qc` 视图，即同样继承不安全 `webPreferences` 与 preload |

**`will-attach-webview` 强制覆盖（@4293633）**

```js
.on("will-attach-webview", (e, webPreferences) => {
  webPreferences.preload = path.join(__dirname, "../preload/index.js");
  webPreferences.contextIsolation = !1;
  webPreferences.webSecurity = !1;
  webPreferences.nodeIntegrationInSubFrames = !0;
  webPreferences.nodeIntegrationInWorker = !0;
  webPreferences.devTools = !!zA().devtool || !1;
  webPreferences.additionalArguments = […, "--zoom"];
})
```

页面里 `<webview>` 的 `preload`/`nodeintegration` 属性被**无条件覆盖**为应用自己的高权限 preload，页面无法通过属性收紧权限（但也不必收紧——页面自己就是受益者）。

**`setWindowOpenHandler` 无条件放行（@4293970）**

```js
.setWindowOpenHandler(() => ({
  action: "allow",
  overrideBrowserWindowOptions: { …, webPreferences: {
      preload: path.join(__dirname, i), contextIsolation: !1, webSecurity: !1, … } }
}))
```

没有 `new URL(details.url)` 检查、没有 `details.disposition` 检查。页面里 `window.open("https://attacker/")` 会得到一个挂载应用 preload 的新 BrowserWindow。

### 1.2 影响

- 任意 XSS / 任意第三方 iframe / 任意被打开的外部网页，都直接持有主进程桥接（见第 3 节），等价于「浏览器内一个页面 = 本机用户权限代码执行」。
- `--no-sandbox` 让 Chromium 渲染进程逃逸类漏洞的价值降为 0：渲染进程本身就是普通 OS 进程，无沙箱可逃。
- `webSecurity:false` 关闭同源策略，跨源读取（`file://`、`local://`、内网 http）不需要任何 CORS 许可；`webviewTag:true` 让页面自建 webview 并自动获得 preload。

### 1.3 复现条件

任意一个已加载页面（含远端课程页、`dialog.html?url=…` 打开的任意站点、`window.open` 出的站点）的 DevTools 控制台或注入脚本即可执行第 3 节 PoC。开发者工具默认关闭（`devTools:!!zA().devtool`），但攻击者不需要 DevTools——远程页面本身就是执行环境。

### 1.4 修复建议

1. 全量改为 `contextIsolation:true`、`sandbox:true`、`nodeIntegration:false`，删除 `--no-sandbox`；确实需要关闭沙箱的机器再单独放行。
2. 桥接改用 `contextBridge.exposeInMainWorld`，按窗口白名单暴露最小方法集（见第 3 节）。
3. `setWindowOpenHandler` 改为：非 `https://*.up366.cn` 一律 `action:"deny"` 并交给 `shell.openExternal`（仍需协议白名单）；或仅 `deny`。
4. 删除 `will-attach-webview` 中的 preload 注入（`webviewTag` 建议整体关掉），远端内容用 `<webview>` 时不带任何 preload。
5. `nodeIntegrationInSubFrames`/`nodeIntegrationInWorker` 关闭——不需要 Node 的子框架不应跑 preload。

---

## 2. 自定义协议

### 2.1 证据

**特权注册（main/index.js@3523610）**

```js
protocol.registerSchemesAsPrivileged([
  { scheme: "local",    privileges: { secure: !0, standard: !0, bypassCSP: !0, corsEnabled: !0 } },
  { scheme: "redirect", privileges: { secure: !0, standard: !0, corsEnabled: !0 } }
])
```

**`local` 流式处理器（main-window.js@4303557，注册函数 `Ube`）**

```js
protocol.registerStreamProtocol("local", (request, callback) => {
  let A = DA(request.url), n = "", s = fs.existsSync(`${A}.u3enc`);
  if (!s && fs.existsSync(A)) {
    // Range 支持：statSync → createReadStream(A,{start,end})
  } else if (s || vee(A)) { /* .u3enc 解密流 px()/dg() */ }
  else callback({ statusCode: 404 })
})
```

**路径解析 `DA`（main-window.js@513908）**

```js
function DA(r, e = "") {
  let i = new URL(r), A = decodeURI(i.pathname),
      n = process.platform == "win32" ? `${i.host}:` : `/${i.host}`;
  switch (i.host) {
    case "flipbooks": n = join(cfg.saveDir, "flipbooks"); break;
    case "res":       n = join(gn(), "res");            break;   // gn() = <安装目录>\resources
    case "resources": n = gn();                         break;
    case "path":      return A.endsWith(".u3f")
                        ? join(cfg.saveDir, "office/fonts", A)
                        : A.replace(/^\//, "");                  // ← 直接返回绝对路径
    case "fonts":     return join(gn(), "fonts", A);
    case "cache":     n = cfg.saveDir;                  break;
    default: … break;
  }
  return e ? join(n, A.replace(/[^/]+$/i, ""), e) : join(n, A);   // ← 未做 realpath/前缀校验
}
```

`decodeURI` 在 `join` 之前执行，因此 `%2e%2e%2f` 会被还原成 `../`。`path.join` 会规范化 `..`，从而跳出预期根目录。

**`redirect` 字符串处理器（@4304959）**

```js
protocol.registerStringProtocol("redirect", async (request, callback) => {
  let u = new URL(request.url);
  Ie.login.getServiceTicket(u.searchParams.get("service") ?? "", !0)
    .then(n => callback({ data: JSON.stringify(n) }))
});
```

同一处的 `webRequest.onHeadersReceived` 给**所有**响应的 `Set-Cookie` 追加 `"; Secure; SameSite=None"`，`onBeforeSendHeaders` 把 UA 改写成 `PC-Up366-Student 6.11.0`（@4305000 附近）。

### 2.2 影响

1. **任意文件读（绝对路径）**：`local://path/C:/Windows/win.ini` → `pathname = "/C:/Windows/win.ini"` → 去首斜杠 → `existsSync("C:/Windows/win.ini")` 为真 → 流式返回。这不是穿越，是**设计上的「路径直通」**（插件加载就依赖它：`loadScript("local://path/" + t.renderer)`，main-window.js@3947053）。
2. **任意文件读（目录穿越）**：`local://res/..%2f..%2f..%2f..%2fUsers%2f<user>%2fAppData%2fRoaming%2fup366%2fconfig.json`、`local://cache/..%2f..%2f…`、`local://flipbooks/..%2f…` 同理；`fonts` 分支同样只 `join`。
3. **跨源可读**：`corsEnabled:true` + 页面 `webSecurity:false` 双重奏，远程页面 `fetch("local://path/C:/…")` 直接拿到字节，可用于窃取 `config.json`（含 `saveDir`、可能含 token/cache 路径）、日志、本地教材、以及任何用户可读文件。
4. **`bypassCSP`**：`res/index.html` 本身没有 CSP（实测文件头只有 title/script/link），bypassCSP 让「以后加的 CSP」形同虚设；即使加了 CSP，`local://` 页面仍可 `eval`/`new Function`/`inline script`。
5. **`redirect://`**：任何能发请求的上下文都能用任意 `service` 换取 CAS service ticket，响应体（JSON）可被远程页面读取（`corsEnabled`）。若把 `service` 指向应用自身的服务地址，等于把当前用户的 SSO 票据交给调用方，可重放到服务端换取会话。
6. 处理器完全不校验 `request.url` 的来源（`referrer`/`Origin`）、不校验 `Range` 头合法性（`Number.parseInt` 失败会得到 `NaN` 并触发 `createReadStream({start:NaN})` 异常），健壮性也差。

### 2.3 复现条件

- 任意窗口/视图内（含远端页面）：
  ```js
  await (await fetch("local://path/C:/Windows/win.ini")).text()
  await (await fetch("local://res/..%2f..%2f..%2f..%2f..%2fWindows%2fwin.ini")).text()
  await (await fetch("local://cache/../config.json")).text()
  ```
- `redirect`：`fetch("redirect://x/?service=" + encodeURIComponent("https://<app-service>")).then(r=>r.json())`。

### 2.4 修复建议

1. `DA()` 对每个 host 分支计算 `root = path.resolve(...)`，随后 `target = path.resolve(root, "." + pathname)`，并要求 `target === root || target.startsWith(root + path.sep)`（**带分隔符**），再 `fs.realpathSync` 后复查（防符号链接）。
2. `case "path"` 删除绝对路径直通；插件加载改为显式白名单目录 + 相对于插件根的解析。
3. `local` 去掉 `corsEnabled` 与 `bypassCSP`（如页面确实需要 eval，单独走不受信任的 scheme）。
4. 处理器内校验来源：只接受 `file:///…/resources/res/` 与自身 app 页面；对跨源请求返回 403。
5. `redirect` 只允许固定 `service` 白名单（应用自身域名），拒绝来自非应用页面的调用；不要回传 ticket 原文，改为服务端一次性 code。
6. `onHeadersReceived` 无条件加 `Secure` 会影响 `http://` 调试环境，建议按 host 过滤。

---

## 3. preload 暴露面

### 3.1 证据

**没有 contextBridge，直接挂 window**（preload/index.js@3634285）

```js
registerExtensions(name, target, raw = !1) {
  let i = raw ? target : this.extensions[name] = new XI(name, target);
  if (process.type === "renderer") { window[name] = i; raw || i.init(); }
  else global[name] = i;
  …
}
```

**注册清单（main-window.js@4289519 / preload/index.js@4127619，两侧一致）**

`AppInfo, Login, CurrentWindow, NetBrowser, DeviceTest, jsBridgeChapterObj, jsBridgeObj, jsChapterObj, Book, jsObj, jsBridgeDictObj, jssdk2023, jssdk2025, LocalBrowser, PluginManager`

传输机制：`ipcRenderer.send(name, …)` / `sendSync(name, …)`，主进程 `ipcMain.on(name, …)`（preload/index.js@3631838、@3633799）。通道名即对象名，**无 per-window 白名单、无 `senderFrame` 来源校验**。

**方法白名单（装饰器提取，共 258 个）**：`Ve`(jsObj) 78、`pA` 22、`lr`(AppInfo) 30、`en`(Login) 22、`AA`(CurrentWindow) 21、`Os` 15、`kA` 16、其余为数据/书籍类。

**高危方法（逐个读过实现）**

| 方法 | 偏移 | 实现要点 | 能力 |
|---|---|---|---|
| `jsObj.readFile({path})` | @4091380 | `existsSync(t) ? readFileSync(t,{encoding})` | **任意文件读**（无根目录校验） |
| `jsObj.writeFile({path,data,flag})` | @4090769 | `checkResourceRootDir(path)` → `writeFileSync` | 写 `%saveDir%\resources\*`（前缀绕过见第 9 项） |
| `jsObj.saveFile({dest,content,enc})` | @4248792 | `FQ(dest)`；`enc` 为假时 **明文** `writeFileSync(dest,content)` | **任意文件写**（`FQ` 对绝对路径直接放行） |
| `jsObj.copyFile({src,dest})` | @4090946 | `copyFileSync(src,dest)`，`dest` 可被另存对话框覆盖，`src` 完全不校验 | 任意读 + 任意写组合 |
| `jsObj.fileRm({dest})` | @4249125 | 仅要求 `/up366/i.test(path)` → `rmSync(recursive:true)` | 递归删除任何路径含 `up366` 的目录 |
| `jsObj.rename({oldPath,newPath})` | @4091886 | 只 `checkResourceRootDir(newPath)` | 把任意文件移入资源目录 |
| `jsObj.unzip({zipFilePath,targetPath})` / `zip` | @4091982 | 只校验 `targetPath`；zip 内容不校验 | 资源目录内任意释放 |
| `jsObj.openAttachment({path})` | @4052878 / @4103299 | `shell.openExternal(e.path)`，`/^http/i` 之外直接透传 | **任意 URI/协议/可执行文件启动** |
| `jsObj.openBrowserWithUrl({url})` | @4101179 等 6 处 | `shell.openExternal(e.url)` | 同上 |
| `jsObj.request({url,data,header})` | @4098632 | `Ie.get/post(config.url(url), …, {headers:e.header})` | **主进程 HTTP 代理**（SSRF / 任意头） |
| `jsObj.downloadFile({fileList:[{url,filePath,unzip}]})` | @4098861 | `filePath` 直用；`unzip` 默认 `true` | 任意路径下载 + 解压 |
| `jsObj.writeFile`/`mkdir`/`exists`/`stat` | @4090600-@4091600 | 组合可做任意探测 | 本机文件/目录指纹 |
| `CurrentWindow.openUrl({url,title})` | @4062477 | 拼 `dialog.html?url=<任意 URL>` 建新窗 | 把任意站点装进特权窗口 |
| `CurrentWindow.showLoading({msg})` | @4101349 / @4251339 | `senderFrame.executeJavaScript(\`showInjectMask("${e.msg || "加载中..."}")\`)` | 模板串未转义，可闭合引号注入 JS（第 10 项） |
| `AppInfo.httpRequest` | @4191537 | `Ie.get/post(config.url(url))` | SSRF |
| `Login.getUserInfo()` / `getToken()` | @4144607 | 读 `Config/UserInfo`、`Config/Token` | **明文密码 / TGT 直读**（第 8 节） |
| `PluginManager.load` / `getPlugin` | @3946337 / @3947053 | 主进程 `require`/`vm.runInThisContext` | 见第 6 项 |

**`shell` 调用点共 22 处**（`openExternal` 10 + `openPath` 12），其中 6 处 `openBrowserWithUrl`、1 处 `openAttachment` 直接把渲染进程入参交给 `shell`。

### 3.2 影响

这是本次评估的**总枢纽**：只要拿到其中任意一个页面的 JS 执行，攻击者就获得

- 本机任意文件读（`readFile`）→ 窃取 token/缓存/教材/浏览器数据；
- 本机任意文件写（`saveFile`/`downloadFile`/`copyFile`）→ 覆盖启动项脚本、写入插件目录（升级为代码执行）、写入用户目录下的任意文件；
- 任意 URI 启动（`openAttachment`/`openBrowserWithUrl`）→ `file:///…exe`、`ms-msdt:`、`search-ms:`、`smb://` 等协议处理器链，等同命令执行；
- 主进程网络位置（`request`/`httpRequest`）→ 内网探测/横向，附带应用 Token 头。
- 目录级破坏（`fileRm` 命中 `up366` 子串即可递归删除）。

### 3.3 复现条件

在任意应用页面（主窗口、`dialog.html` 里 `<webview>` 加载的任意站点、`window.open` 出的站点、jssdk 课程页）执行：

```js
await window.jsObj.readFile({ path: "C:/Windows/win.ini" });
await window.jsObj.saveFile({ dest: "C:/Users/Public/up366_poc.txt", content: "x" });
window.jsObj.openAttachment({ path: "file:///C:/Windows/System32/calc.exe" });
window.jsObj.openBrowserWithUrl({ url: "ms-settings:privacy" });
await window.jsObj.request({ url: "http://127.0.0.1:8080/", header: { "X-Poc": "1" } });
```

无需登录态、无需用户交互（`openAttachment` 会弹系统确认，但 `saveFile`/`readFile`/`request` 完全静默）。

### 3.4 修复建议

1. `contextIsolation:true` + `contextBridge.exposeInMainWorld`，每个窗口只暴露自己需要的少数方法（主窗口不需要 `PluginManager`/`Book`；课程视图不需要 `readFile`/`saveFile`）。
2. 主进程侧按 `event.senderFrame.url` 做**白名单校验**（只允许 `local://res/**`、`https://*.up366.cn/**`），对远端 frame 直接拒绝所有写/执行类方法。
3. `readFile`/`saveFile`/`copyFile`/`fileRm` 统一走 `resolveInside(root, p)`（`path.resolve` + 分隔符前缀 + `realpath`），根目录只允许 `saveDir`/`CurrentUser.cacheDir`。
4. `openAttachment`/`openBrowserWithUrl` 改为协议白名单（`https:`, `http:` 仅限已知域；`file:` 仅限 `saveDir` 下的 pdf/doc 等文档扩展），禁止 `file:` 指向 `.exe/.bat/.ps1/.lnk`，并拒绝 `ms-`/`search-`/`smb` 等。
5. 删除 `request`/`httpRequest` 这类通用代理，需要网络能力时在主进程定义具体业务方法（固定 URL 模板）。
6. `downloadFile` 的 `filePath` 一律 `join(saveDir, …)` 重写，忽略入参；`unzip` 前做条目名校验（拒绝绝对路径与 `..`）。

---

## 4. 命令执行 / 外部程序

### 4.1 证据

**更新器启动（数组传参，未走 shell，main-window.js@4168747）**

```js
t.push("-pid", String(process.pid), "-f");
for (a of updateItems) t.push(a.localUpdateFile());
t.push("-d");  for (a of updateItems) t.push(a.destPath());
t.push("-mod", id(!1), "-v");
for (a of updateItems) t.push(a.versionFilePath() || "#", String(a.updateVersionNo()));
process.platform === "win32" || "darwin" ? t.push("-m", nc(), i ? "-re" : "-ep") : …
```

`nc()` = `process.execPath`（main-window.js@512629），即用当前 exe 自身调用更新器（`-m` 是 model 路径）。**参数是数组，无 shell 拼接，无可注入点。**

**Linux 分支（main-window.js@540435）——shell 字符串**

```js
fs.writeFileSync("/tmp/up366update",
  ` tail --pid=${process.pid} -f /dev/null "${process.argv.join('" "')}"     `);
exec("bash /tmp/up366update")
```

`process.argv` 直接进入双引号包裹的 shell 命令；argv 中含 `"` 或 `$(…)` 即可注入。

**更新器权限提升（同处及 @4171081）**

```js
let e = `${CA("up366update")}.exe`;                       // 资源目录里的随包更新器
let t = join(cfg.saveDir, "update", "exe", "up366update.exe");
if (fs.statSync(e).isFile() && fs.existsSync(t) && await La(e) === await La(t)) return {success:!0, exeFileName:t};
```

`La()` = MD5（@517748 `crypto.createHash("md5")`）。随包更新器被复制到 `%saveDir%\update\exe` 并做 MD5 一致性比对——**这一步是好的**；但随后对更新包调用该 exe 时带 `-ep`（elevate prompt），UAC 提示用文案「请允许使用管理员权限」（@4171081），即更新以管理员权限落盘。

**其它 `child_process`**

| 位置 | 内容 | 判定 |
|---|---|---|
| @4013325 | `startupSpooler`：`let s=[...process.argv,"--cmd=startup-spooler"]; e += `"${i}"`; exec(e,{name:"up366"})` | argv 拼 shell 字符串，Windows；本地参数注入，低危 |
| @4171338 | `execSync(`chmod +x "${t}"`)` | Linux only，路径来自 saveDir，低危 |
| @508583 / @523982 等 | `libup366.node` 原生模块 + `systeminformation`（`exec/execSync` 遍布 @3581045-@3772209） | 命令字符串固定，未见入参拼接；但引入了大量常驻 exec 面 |
| 其余 | 打包的 `node-7z`（`spawn`）、`ffmpeg` 封装（`spawn`） | 路径参数来自业务数据，需结合第 3/9 项的写权限利用 |

**`shell.openExternal` 才真正是「外部程序执行」主通道**（见第 3 节）：`file:///C:/Windows/System32/calc.exe` 在 Windows 上直接执行；`search-ms:`/`ms-msdt:` 可走系统处理器链。

### 4.2 影响

- 更新器本身调用方式正确（数组），不构成注入。
- Linux 分支的 `bash /tmp/up366update` 是明确命令注入，但仅 Linux + 需要控制启动参数。
- 真正的「执行任意程序」能力来自 `shell.openExternal` 的两个无过滤入口，属于第 3 节的一部分。

### 4.3 复现条件

- Windows：`jsObj.openAttachment({path:"file:///C:/Windows/System32/calc.exe"})`。
- Linux：`up366 --x='$(id>/tmp/pwn)'` → 触发 `relaunch` 路径写出 `/tmp/up366update` 并 `bash` 执行。

### 4.4 修复建议

1. Linux 分支改为 `spawn("bash", ["-c", script], …)` 且用 `shell-quote` 转义，或直接 `process.argv` 数组传给 `spawn`（去掉 `bash -c`）。
2. `startupSpooler` 同样改为 `spawn(process.execPath, argv, {shell:false})`。
3. `shell.openExternal` 前统一 `new URL()` 解析 + 协议白名单 + 扩展名白名单（见 3.4-4）。

---

## 5. 更新链

### 5.1 证据

**检查更新（@3937406 `checkUpdate` / @3938000 `checkUpdateFromInfo`）**

```js
let A = await Ie.other.getUpdateConfig(modelName, username, String(localVersionNo()));
if (A.result.code === 0 || A.result.code === 100) {
  this.updateInfo = A;
  this.hasUpdate() && this.updateVersionNo() == i && (this.updateInfo.data.status = 0);
}
```

`hasUpdate()` 只看 `updateInfo.data.status === 1`（服务端下发），客户端唯一的「拒绝」条件是 **版本号相等**，没有任何 `serverVersion > localVersion` 比较。列表接口 `checkUpdateList`（@4164678）走 `checkVersionList` / `miniCloudUpdateList`，同样只取 `lastVersion.status`。

**下载（@3938800 `downloadUpdateFile` → `startDownload`）**

```js
this.updateInfo?.data?.url.startsWith("http://") &&
  this.updateInfo.data.url.replace("http://", "https://");        // ← 结果被丢弃，空操作
this.download = Ie.download(this.updateInfo.data.url, e, void 0,
  { breakpoint: !0, md5: this.updateInfo?.data?.apkMd5, useLocalBackDownload: !0 });
```

**校验**：只有 `md5`（`La()`/`Pp()` 为 `createHash("md5")`，@517748），且 md5 与 url 来自**同一份**服务端响应。

**安装**：`quitAndInstall()`（@4168747）→ 校验 `up366update.exe` 与资源内副本 MD5 一致 → 以 `-ep` 提权执行，参数 `-f <包> -d <目标> -v <版本文件> <版本号> -m <exe> -ep`。

**`lib\ia32\win32\version`**：`pg()`（@513363）返回 `<安装目录>\lib\<arch>\<platform>`；`gn()` 返回 `<安装目录>\resources`。`-v <versionFilePath>` 指向的就是这类版本标记文件（`ExecutableUpdate.versionFilePath()` = `join(destPath(),"version")`，@3944392），由更新器写回安装目录下的版本号（当前 `1.0.0.10`）。它的唯一用途是给下一次 `localVersion()` 提供「本地版本」，进而参与第 5.1 的「相等即不更新」判断——**因此它既是升级标记也是降级判定的唯一依据**。

**插件包（@3948684 `Qq`/`kN`）**：`destPath() = join(cfg.saveDir, "plugins")`，下载 → `unzip` → `version` 写入 → 主进程 `require`（见第 6 项）。

### 5.2 影响

1. **无签名校验**：安装包只校验 MD5。MD5 有可行碰撞，且 MD5 值与下载 URL 来自同一响应——控制响应的一方（服务端被拿下、内网 DNS 劫持、代理）可以同时给出恶意包与匹配 MD5。
2. **http 升级是死代码**：`replace(...)` 的返回值被丢弃，若服务端返回 `http://…`，包就是明文 HTTP 下载（下载器是否跟随重定向到 http 亦无校验）。
3. **无防降级**：只要服务端说 `status=1` 且版本号 ≠ 本地，就会下载安装。攻击者（或服务端配置错误）可以让客户端回滚到含已知漏洞的旧版本，再配合第 1/3 项利用。
4. **提权**：更新器以管理员权限运行（UAC 提示文案明说），恶意包 = 管理员权限落盘。
5. 版本文件 `lib\ia32\win32\version` 在安装目录（`Program Files` 下），普通用户不可写，这条链路本身没有本地降级入口；但 `saveDir` 下的插件/资源可写（第 6 项）。

### 5.3 复现条件

需要控制更新接口响应（HTTP 中间人 / 服务端被控 / 代理）或在 `%saveDir%\update\` 预置包并让 `isDownloaded()` 命中（@3938800 附近：`existsSync(e) && La(e) === apkMd5`——MD5 已知即可命中，无需真实下载）。随后触发 `checkClientUpdate`（`CurrentWindow.checkClientUpdate` / 启动时 `checkStartUpdate`）。

### 5.4 修复建议

1. 删除 `replace` 空操作，改为显式拒绝非 `https` URL；下载器强制 `https` 且禁止 `http` 重定向。
2. 换 MD5 为 **Ed25519/RSA 签名**（公钥内置），或至少 SHA-256 + 签名清单；校验失败不落盘、不执行。
3. 客户端比较语义版本，禁止低于本地版本；对 `-v` 版本文件同时校验单调递增。
4. 更新器本身已有 MD5 一致性校验，建议升级为 SHA-256 + Authenticode 校验（`WinVerifyTrust`），并在提权前再次校验签名主体。
5. 更新包解压目标固定为安装目录，拒绝 `..`/绝对路径条目。

---

## 6. fuse / 完整性保护

### 6.1 证据（静态 + 既有运行观测）

- 归档格式 `app.u3a` 是厂商改过的 asar（虚拟目录 `fs.statSync(...).isDirectory()===true`，见 `recon/progress.md`），带自定义完整性串 `Integrity check failed for u3a archive`（exe `0x6af1a6c`）与 fuse 串（`0x6af73b5`）。
- 运行通道矩阵（`recon/runtime-access.md`，2026-11-01 实测）：
  | 通道 | 结果 |
  |---|---|
  | `--inspect=9229` | 被禁（无监听） |
  | `--remote-debugging-port=9222` | 被禁（无监听） |
  | `NODE_OPTIONS=--require` | 被禁（marker 缺失） |
  | `ELECTRON_RUN_AS_NODE=1` | **可用** |
- `ELECTRON_RUN_AS_NODE=1 up366.exe -e "…"` 直接得到 Node 16 进程，且 `fs` 被厂商打过补丁：读 `app.u3a/*` 返回归档内明文、读 `x.js` 自动回落到 `x.js.u3enc`（`recon/progress.md`）。
- `.u3enc` 为 AES-128-CBC，密钥硬编码在 bundle 中：`Buffer.from("QJBNiBmV55PDrewyne3GsA==","base64")`（main-window.js@515903 附近 `dg()`）。

### 6.2 判定：哪些保护实际生效

| 保护 | 状态 | 依据 |
|---|---|---|
| 禁用 `--inspect`（`EnableNodeCliInspectArguments`） | **生效** | 端口无监听 |
| 禁用 `--remote-debugging-port` | **生效** | 端口无监听 |
| 禁用 `NODE_OPTIONS`（`EnableNodeOptionsEnvironmentVariable`） | **生效** | marker 未写入 |
| `ELECTRON_RUN_AS_NODE`（`RunAsNode`） | **未关闭 = 完整绕过** | 可直接以 Node 身份运行同一二进制，读归档明文 |
| 归档完整性（`Integrity check failed for u3a archive`） | **只防「改坏了」，不防「读出来」** | 校验针对归档结构/条目；RunAsNode + 补丁 fs 已读到全部明文（`extract/app.u3a` 14 文件 17.5 MB） |
| `.u3enc` 内容加密 | **不提供机密性** | 对称密钥内置在明文 bundle 中，密钥即公开 |
| asar 完整性 fuse（Electron 侧） | 版本相关，Electron 22 对 `EnableEmbeddedAsarIntegrityValidation` 支持不完整；本应用用的是自有 `u3a` 校验 | 未在 22.3.27 找到对应 fuse 行为证据 |

结论：这层保护的真实作用是**防误改/防简单替换**，对本地攻击者与「读代码」目标**完全无效**；而它带来的成本是 `--no-sandbox` + 自定义 fs 补丁 + 大量协议特例，反而扩大攻击面。

### 6.3 影响

- 攻击者可离线获得全部源码与前端资源（已发生），并据此定位第 1-5 项的所有入口。
- 完整性校验只覆盖归档，不覆盖 `<安装目录>\resources\app.json`（`ignoreCertificate`）、`%saveDir%\plugins`、`%saveDir%\resources`——这些都可写且会被加载（第 1/6 项）。

### 6.4 复现条件

`ELECTRON_RUN_AS_NODE=1 up366.exe -e "console.log(require('fs').readFileSync('<install>/resources/app.u3a/dist/main/index.js','utf8').slice(0,64))"`（已在 `recon/progress.md` 记录）。

### 6.5 修复建议

1. 关闭 `RunAsNode` fuse（`npx @electron/fuses write --app up366.exe RunAsNode=off`），并同时关闭 `EnableNodeCliInspectArguments`、`EnableNodeOptionsEnvironmentVariable`、`OnlyLoadAppFromAsar` 打开。
2. 归档完整性校验覆盖**全部**可加载资源（`app.json`、`plugins`、`resources`），失败即拒绝启动，不要只覆盖 `u3a`。
3. 若确需保护代码，接受「本地不可防」的现实：把机密（密钥、证书）移到服务端，不要内置对称密钥。
4. 用官方 asar integrity（`EnableEmbeddedAsarIntegrityValidation`）+ 代码签名，而不是自研格式。

---

## 7. Electron 22.3.27 / Chromium 108 版本风险

### 7.1 证据

`recon/runtime-access.md` 实测 `ELECTRON_RUN_AS_NODE=1 up366.exe -e "process.versions"` → `22.3.27 | 16.17.1 | undefined`。Electron 22 对应 Chromium 108（2022-11），Electron 22 的官方支持在 2023 年结束，此后不再获得 Chromium 安全补丁。

### 7.2 与本应用可达路径相关的漏洞类别（不列 CVE）

| 类别 | 可达性 | 说明 |
|---|---|---|
| Chromium 渲染引擎内存破坏（V8/Blink UAF、类型混淆） | **高** | 应用主动加载任意远端页面（`openWebView`/`dialog.html?url=`/`window.open` 全放行），攻击者只需让用户打开/被打开一个页面；且 `--no-sandbox` 让渲染进程崩溃/利用直接等于 OS 进程权限 |
| 导航 / window-open 处理缺陷 | **高** | `setWindowOpenHandler` 全 allow，Electron 22 中 `nativeWindowOpen` 相关历史问题（新窗口绕过 `will-navigate`/`webPreferences` 覆盖）直接适用 |
| 自定义协议实现缺陷 | **高** | 应用自建 `local`/`redirect` 两个 scheme，Electron 22 的 `registerStreamProtocol` 无内建路径校验（第 2 项已实证） |
| `webview` 标签问题 | **高** | `webviewTag:true` + `will-attach-webview` 强制注入 preload；历史上 webview 的 `preload` 逃逸/上下文混淆问题在 22.x 未完全修复 |
| `shell.openExternal` 参数/协议注入 | **高** | 应用直接把不可信字符串交给 `shell`（第 3/4 项） |
| Chromium 已废弃特性（`allowRunningInsecureContent`、混合内容放宽） | 低 | 未显式开启；`webSecurity:false` 已覆盖同类风险 |
| Node 16.17.1 自身漏洞（`node:fs`、`vm`、`zlib`） | 中 | 主进程直接处理不可信 zip/网络数据（`node-7z`、`Ie.download`），Node 16 已 EOL，无补丁 |

### 7.3 影响

「老版本 Chromium + 无沙箱 + 主动加载远端页面 + 页面直连主进程桥」是四重叠加：即使不使用第 1-6 项的应用层缺陷，单靠一个 Chromium 渲染漏洞即可拿到本机用户权限；反之，即使 Chromium 无漏洞，第 1-6 项也已足够。

### 7.4 复现条件

需要一个被加载的恶意页面（课程内容、外链、被劫持的 CDN/更新包、或 `openWebView` 打开的任意 URL）；本机无沙箱，成功即代码执行。

### 7.5 修复建议

1. 升级到仍有安全支持的 Electron 版本（当前为 3x 线），并跟住 Chromium 安全版本。
2. 恢复沙箱（`sandbox:true`，去掉 `--no-sandbox`），这是抵御渲染漏洞的唯一结构性措施。
3. 远端内容与本地特权页面**分进程分 session**：远端页面用独立 `partition`（无 preload、无 Node、`webSecurity:true`、有 CSP）。
4. 给 `res/index.html` 及各 HTML 加严格 CSP（`default-src 'self'; script-src 'self'; object-src 'none'`），并在 `local` scheme 上取消 `bypassCSP`。

---

---

## 8. 凭据处理（与 api 交叉复核）

### 8.1 证据

**(a) 明文密码落盘（main/index.js@3886185，等价实现 main-window.js@4144155）**

```js
await zD(this, { mode: "LoginByUsername", type: nQ.LoginByUsername,
                 username: e.userName, password: e.password }),
e.remember
  ? iA.save("Config", { name: "UserInfo",
      value: JSON.stringify({ username: e.userName, password: e.password }) })
  : iA.remove("Config", { name: "UserInfo" })
```

`e.password` 是 IPC 传入的原始明文（登录接口 `Ge.login.login(e.userName, e.password)` 直接使用）。落库封装在 `{saveDir}\setup.cfg`（SQLCipher SQLite），密钥常量为 `LZ = "{3E2A243F-7B8B-45C8-9E7D-885A891820A7}"`（main/index.js@3527022），实际使用点：

```js
this.dataBase.pragma(`key="${LZ}${t ?? ""}"`)     // main/index.js@3529280 (initDb)
this.dataBase.pragma(`rekey="${LZ}${t ?? ""}"`)   // main/index.js@3529190
```

**(b) 凭据可被任意渲染进程直读（main-window.js@4144607，类 `en` = `Login` 桥）**

```js
getUserInfo(){ let e = iA.get("Config", { name:"UserInfo" })?.value; return e ? JSON.parse(e) : void 0 }
getToken(){ return iA.get("Config", { name:"Token" })?.value }
```

两者都在 RPC 白名单内（`_([...], en.prototype, "getUserInfo", 1)`），类 `en` 以 `"Login"` 名注册进 preload（main-window.js@4289519）。**没有 `event.senderFrame` 来源校验**。

**(c) 直播子进程命令行泄凭据（main/index.js@3702650–@3703560）**

```js
let i = { userid: ..., token: qt(`${String(CurrentUser.info.uid)}${e.pwd}up366`),
          pwd: e.pwd, roomid: e.roomId, roomname: ...,
          up366UserName: CurrentUser.info.account,
          up366Password: CurrentUser.loginInfo.mode == "LoginByUsername"
                           ? CurrentUser.loginInfo.password : "",
          up366Openid: ..., up366Token: ...,
          machineCode: global.ComputerInfo.MachineCode, UserSig: e.UserSig, appId: e.sdkAppId, ... };
let n = ""; for (let s in i) n += `${s}=${i[s] ?? ""}&`;
n = Buffer.from(n).toString("base64");
n = wc(n, ["+","/","="], ["-","_","."]);   // base64url
n = `up366course://?${n}`;
this.update.startupMainExe([n]);           // 作为 argv 拉起直播组件
```

`qt()`/`Rt()` 均为 `crypto.createHash("md5")`（main-window.js@515903），即 token = `md5(uid + 房间密码 + "up366")`，盐硬编码。

### 8.2 影响

1. **凭据直读是主问题**：不需要读盘、不需要解 SQLCipher，任何页面执行 `window.Login.getUserInfo()` 就拿到 `{username, password}` 明文，`window.Login.getToken()` 拿到 TGT。与第 1/2 项叠加后，任意远程页面可静默窃取账号密码（撞库、异地登录、换取 `u3t` 长期访问）。
2. **落盘等于明文**：SQLCipher 密钥硬编码在明文 bundle 中（与 6.2 的 `.u3enc` AES 密钥同一类问题），任何能读 `setup.cfg` 的进程 / 备份 / 同步盘都能解密。
3. **命令行泄密**：Windows 下 `up366course://?<base64url>` 出现在直播组件命令行，同机任意进程（含低权限进程、ETW、崩溃转储、进程监控）都可读到并解码出明文密码、房间密码、UserSig、machineCode。任务管理器级别即可，无需管理员。
4. token 派生用 `md5(uid + pwd + "up366")`，盐公开且无每会话随机量 → 同房间同密码 token 恒定，可重放。

### 8.3 复现条件

- `window.Login.getUserInfo()` / `window.Login.getToken()`（任意窗口/视图，含远端页面）。
- 落盘：勾选「记住密码」登录一次 → `sqlite3` + `PRAGMA key='{3E2A243F-7B8B-45C8-9E7D-885A891820A7}'` 打开 `{saveDir}\setup.cfg` → `select * from config where name='UserInfo'`。
- 命令行：`wmic process get commandline | findstr up366course`。

### 8.4 修复建议

1. `Login` 桥删除 `getUserInfo`/`getToken`，或返回脱敏结果（`{username, hasPassword:true}`）；主进程侧按 `senderFrame.url` 白名单校验。
2. 不存明文密码：只存可撤销、设备绑定的 refresh token；确需本地保存时用 DPAPI（`CryptProtectData`）后再入库；SQLCipher 密钥改由 DPAPI/Keychain 保护的随机密钥派生，不要硬编码。
3. 直播组件凭据改走 stdin/命名管道/服务端换发的一次性 ticket，**绝不进 argv**；命令行只传 sessionId。
4. token 改为服务端签发的短期可撤销票据，不要 `md5(uid + pwd + 固定盐)`。

> 交叉引用：api.md 另记录「`Authorization` 头恒为空、鉴权靠 `u3t`(TGT) + `?ticket=`、`ut` 为 `md5(payload + 公开常量 + md5(ts))` 可离线伪造」。本报告只独立复核了 (a)(b)(c) 三项（偏移见上），协议侧结论以 api.md 为准。

---

## 9. 渲染层注入面与代码执行入口（与 frontend 交叉复核）

### 9.1 证据

**(a) `#/dialog` 用 `innerHTML` 渲染主进程下发的文案（assets/index.a16a3d07.js@25476 / @25766）**

```js
s("div", { class: "title",   innerHTML: e.opts.title })
s("div", { class: "content", innerHTML: e.opts.content })
```

`opts` 来自主进程推送的 `dialog` 事件（同文件 @23101 `CurrentWindow.on("dialog", …)`）。主进程侧大量调用点是 `CurrentWindow.showDialog({type:"info", content: …})`，content 会拼进服务端文案 / 接口 msg / 文件路径（例如 main-window.js@4062800 附近 "找不到文件或文件夹<br>" + 路径）。该 DOM 所在窗口是本应用的 `mM` 对话框（`index.html#/dialog`），带应用 preload 且 `contextIsolation:false`。

**(b) 记住密码的第二份明文副本（static/login/login.bdb7e7c2.js@263816 / @263861）**

```js
Login.saveData("userInfo", JSON.stringify({ userName: …, password: … }))
```

启动时回填 `this.form.password`（@264933 `JSON.parse(Login.loadData("userInfo"))` → @265002 `this.form.password=n.password` 回填）。走 `NetBrowser.saveData` → 主进程 `Config` store（key `userInfo`），与第 8.1 的 `UserInfo` 是两个独立 key，**都存明文**。

**(c) 任意 URL 进 iframe / webview，且与主窗口同进程同上下文**

- `dialog.html`：`<webview id="frame">` 的 `src` 直接取 `?url=`（`local://` 开头时 `decodeURIComponent`），无协议/域白名单。
- `#/separate?url=`：`<iframe name="iframeMap" src=decodeURIComponent(url)>`，并暴露 `window.loadUrl(u)`。
- `#/login` 父页与 iframe 之间用**同名全局函数**互调，无来源校验：父页 `window.frames.iframeMap[fn](...args)`（login.b1714e53.js@5318 `executeScript(e,i)`），子页把 `onLogin/setUser/setUserData/stuSetUser/getExtendBookData` 挂到 `window` 供主进程 `executeJavaScript` 调用。

**(d) 离线播放器的动态脚本与 eval（xotFullScreen/js/app.1ae30858.js@25167 / @25326 / @885072）**

```js
var resPath = JSON.parse(NetBrowser.getPageParams()).resPath;
this.loadScript("".concat(resPath, "page1.js"), function () {
  var questObj = flipToXot(pageConfig, resPath); … });
…
var data = eval('(' + xhr.responseText + ')');     // ajaxComplete()
```

`loadScript` 是 `document.createElement("script")` + `appendChild`（preload/index.js@3947053 的同名实现），即把 `<resPath>page1.js` 当脚本执行；`resPath` 来自主进程 `getPageParams()`（书籍包内参数），`page1.js` 随书籍包下载，包本身只有 zip 密码、无签名（第 5 节）。`eval` 则把任意 HTTP 响应体当 JS 求值。

### 9.2 影响

1. **(a)** 标准 XSS sink，且落在**特权窗口**：`res/index.html` 无 CSP、`local` scheme 还开了 `bypassCSP`，`<img src=x onerror=…>` 一旦进入 `opts.content` 就等于第 3 节的桥接权限（读写文件、拉起程序）。触发只需「服务端可控的提示文案」，而应用大量文案来自接口 `result.msg` 与本地路径，现实可达。
2. **(b)** 让第 8 节的明文口令问题从一个 key 变成两个；即使以后把 `Config/UserInfo` 改成 DPAPI，`userInfo` 这条仍会泄。
3. **(c)** 把第 2/3 项的「远程页面 → 桥接」从"打开新窗口"降为"主窗口里一个 iframe"：`webSecurity:false` 关掉同源策略后，远程 iframe 可 `parent.jsObj.*` 直接调用，无需用户交互（只要该 URL 被 `/separate?url=` 或 `dialog.html?url=` 载入）。双向同名全局函数还带来跨上下文污染：远程页面可覆盖 `onLogin`/`setUser`，主进程 `executeJavaScript` 时即被劫持。
4. **(d)** 渲染层独立的代码执行路径：控制书籍包（服务端 / 更新链 / 本地写权限）→ 植入 `page1.js` → 打开离线播放器即在带桥接的视图里执行任意 JS，可作持久化后门，不依赖第 1 项的写原语。

### 9.3 复现条件

- (a)：令主进程执行 `CurrentWindow.showDialog({type:"info", content:'<img src=x onerror="window.jsObj.openAttachment({path:&#39;file:///C:/Windows/System32/calc.exe&#39;})">'})`（等价于任何把服务端文案塞进 content 的调用）。
- (c)：导航到 `index.html#/separate?url=https%3A%2F%2Fattacker%2F`，页面内执行 `parent.jsObj.readFile({path:"C:/Windows/win.ini"})`；或 `CurrentWindow.openUrl("https://attacker/","x")`。
- (d)：替换书籍包内 `page1.js`（或让 `resPath` 指向可写目录）→ 打开离线播放器。

### 9.4 修复建议

1. (a) `innerHTML` 改 `textContent`；确需富文本时走白名单 sanitizer（DOMPurify），主进程侧限制 content 只能是纯文本/固定模板；并给 `res/index.html` 加 CSP。
2. (b) 删除 `saveData("userInfo", …password)`，只存用户名与「是否记住」布尔；自动登录改 token（见 8.4）。
3. (c) `#/separate` 与 `dialog.html` 对 `url` 做协议+域白名单（只允许 `https://*.up366.cn`），并在主进程 `openUrl` 入口统一校验；iframe 加 `sandbox` 且不继承 preload；跨上下文调用改 `postMessage` + `event.origin` 校验，不要同名全局函数。
4. (d) 移除 `loadScript`，改静态 import 或 `import()` 白名单路径；`eval('('+responseText+')')` 改 `JSON.parse`；书籍包加签名/哈希清单，`page1.js` 校验通过才执行。

> 交叉引用：frontend.md 另记录 `static/login` 产物把 API 基址硬编码为 `*.up366test.cn`（构建配置问题，非漏洞）、以及存储 key 全表（`USER_INFO`/`userInfo`/`book_extend_data`/`u3h` 等）。本节 (a)(b)(c)(d) 均已按上述偏移独立复核。

## 附：证据复核命令

```bash
python - <<'PY'
s=open(r"H:/project/up366/extract/app.u3a/dist/main/main-window.js",encoding="utf-8").read()
for off in (4291704,4293633,4293970,4303557,4304959,4091380,4248792,4249125,4052878,4098632,3945334):
    print(off, s[off-120:off+260].replace("\n"," "))
PY
```

（`main/index.js` 对应偏移：`3522755` 启动参数、`3523610` scheme 注册、`3523900` loading 窗口。）
