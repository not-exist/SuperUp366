# up366 客户端逆向 — 完整结论

> 目标：`C:\Program Files (x86)\up366\client`（天学网学生端 / 北京天学网教育科技股份有限公司）
> 产物根目录：`H:\project\up366`，全部只读分析，安装目录零写入。
> 时间：2026-11-01

---

## 0. 一句话

这是一份 **Electron 22.3.27 外壳 + 改名 asar 容器 + AES-128-CBC 资源加密 + 自研 mod 包管理** 的客户端；
三套"保护"全部不需要爆破就能绕过，源码可完整还原；真正的风险不在加密，而在**渲染层零隔离 + 258 个无来源校验的桥接方法**，
以及**明文口令落盘并经桥接直接外泄**。

---

## 1. 目标画像

| 项 | 值 |
|---|---|
| 产品 | 天学网学生端（up366），app 版本 **6.11.0**，build `650eeb1` |
| 运行时 | **Electron 22.3.27 / Chromium 108 / Node 16.17.1**（均已 EOL） |
| 体积 | 443 文件 / 635,503,284 B（606.1 MiB） |
| 模块 | 自研 `.mod` 清单：`app 6.11.0` + `app-native` / `app-resources` / `lib-tools` / `lib-up366` / `lib-additional` / `plugin-ases 1.5.0` |
| 原生模块 | `libup366.node`(533KB)、`libsqlite.node`(1.7MB)、`libases.node`(418KB)、`libexcel.node`(1.7MB) |
| 核心插件 | `ases 4.3.1.0` 英语听说评测（Kaldi ASR + LightGBM/libsvm + ONNX），安装体积 **303.3 MB** |
| 用户目录 | 配置 `%APPDATA%\up366`，业务数据 `C:\Up366StudentFiles`（`config.json` 里 `saveDir`） |

---

## 2. 三套保护的真实边界（逐条实测）

### 2.1 `.u3enc` —— 完全攻破，可读可写

```
KEY = base64 "QJBNiBmV55PDrewyne3GsA=="      // 硬编码于 up366.exe@128158069
格式 = [16B IV][AES-128-CBC 密文][PKCS#7]
```

- 43/43 个 `.u3enc` 实测解密通过，明文可打印率 1.000。
- 长度公式 `ct = 16 + 16 * ceil((pt+1)/16)`，43/43 成立。
- **加密白名单只有 5 个后缀**：`.xml .js .html .htm .css`；图片/音频/字体一律明文。
- ECB 与 CTR 均已排除：明文里 15,159 个重复 16B 块，密文重复块为 0。
- 因为密钥和 IV 规则都拿到了，**可以重新加密任意前端资源**——这是"前端控制"的基础。

### 2.2 `app.u3a` —— 就是改名的 asar，条目选择性加密

- 不是自定义容器：Electron `asar` 把 `asar` 字样换成 `u3a`，报错串仍是 `Failed to parse header` / `Integrity check failed for u3a archive`，源码路径仍是 `electron/shell/common/asar/archive.cc`。
- **条目按后缀选择性加密**，用的是和 `.u3enc` 完全相同的原语、同一把密钥；其余后缀明文。所以明文/密文交替，导致最初的卡方统计误判为"未知编码"。
- 实测锚点：4100 明文 `package.json`、8023 明文 HTML、5328..5344 为 IV、5344 起解出 `lib-up366`（~2.7KB）。
- **只有 asar 头部索引没解**（前 8 字节也是密文，`u32le@0` 应为 4 实为 1,492,020,612），判为原生侧另一把密钥。
- **头部没有解的价值**：厂商 fs 层读取时自动解密，`tools/dump_archive.js` 已全量取出 14 文件 17.5 MB。

⚠ 两条防错：CBC 流内任意块边界都能"解出可读文本"（实测 5328 后有 165 个假阳性），**不要按 16 字节对齐枚举条目起点**；**不要按明文 carve**（明文/密文交替）。

### 2.3 `encr` —— ases 插件自带的玩具层

固定 8 字节循环异或 `(i % 8) + 1`，无密钥，只作用于 `plugins/ases` 下的 `.json/.txt`。

### 2.4 Electron fuse

| 保护 | 位 | 实测 |
|---|---|---|
| RunAsNode | 1 | **可用** ← 这是全局突破口 |
| NodeOptions | 0 | `NODE_OPTIONS=--require` 无效 |
| CliInspect | 0 | `--inspect` 无效 |
| EmbeddedAsarIntegrity | 0 | — |

`--remote-debugging-port` 也不生效（无监听、stderr 无 `DevTools listening`），补丁层把 devtools server 去掉了。
**所以唯一可用的运行时通道是 `ELECTRON_RUN_AS_NODE=1`**：拿到一个 Node 16 进程，且用的是厂商打过补丁的 fs。

---

## 3. 拿到了什么（可复现）

| 产物 | 内容 |
|---|---|
| `extract/app.u3a/` | 14 文件 17.5 MB —— `dist/main/index.js` 3.90MB、`dist/main/main-window.js` 4.34MB、`dist/preload/index.js` 4.14MB、`dist/utils/libup366.js`、`dist/res/loading` |
| `extract/res/` | 46 文件 —— Vite 构建产物（index/login/menu/toolbar/upcode/writing-report/dictation-report）+ 老 vue-cli（static/login、micTest、network）+ xotFullScreen |
| `extract/xotFullScreen-src/` | 173 文件 3.25 MB —— 由明文 source map 的 `sourcesContent` 还原的原始 Vue 源码，构建机根 `D:/work/fullscreen-html` |
| `extract/plugins/ases/` | 解密后的插件 JS + 特征表 JSON |
| `recon/*.md` | api / security / frontend / ases / crypto / inventory / progress |

复现命令：
```bash
ELECTRON_RUN_AS_NODE=1 "…/up366.exe" -e "$(cat tools/dump_archive.js)"   # 解 asar
node tools/u3enc.js decrypt <in.u3enc> <out>                              # 解单个资源
node tools/u3enc.js encrypt <in> <out.u3enc>                              # 反向加密
```

---

## 4. 架构

### 4.1 进程与窗口

渲染侧是多窗口结构，每个窗口都是 `local://res/index.html#<route>`：

```
#/menu  #/toolbar  #/dialog  #/login  #/  #/offlinebar  #/separate  #/pdf/*
                          └─ 登录窗口内嵌 iframe → local://res/login.html
```

安全相关配置（所有窗口一致）：
```js
contextIsolation: false,
webSecurity:      false,
nodeIntegrationInSubFrames: true,
nodeIntegrationInWorker:    true,
webviewTag:       true,
// 全局 --no-sandbox --no-zygote
// local scheme: { secure:true, standard:true, bypassCSP:true, corsEnabled:true }
```

### 4.2 自定义协议

| 形式 | 解析 |
|---|---|
| `local://resources/<rel>` | 安装目录 `resources` 下 |
| `local://flipbooks/<rel>` | 书本目录 |
| `local://<盘符去冒号>/<路径>` | **任意绝对路径** |
| `redirect://getServiceTicket/?…` | CAS ticket 中转 |

### 4.3 前端

6 个 HTML 入口、**12 条 hash 路由**、11 个存储 key。
Vite SPA 自身 **0 条直连 HTTP**——业务全部走 preload 暴露的全局对象（`Login` / `AppInfo` / `CurrentWindow` / `NetBrowser`）。
三个遗留页（Vue2 + 自家 u3- 组件库）直连 25 条 API。
离线播放器 `xotFullScreen` 只有离线入口：`NetBrowser.getPageParams().resPath` → `loadScript(resPath + "page1.js")` → 全局 `pageConfig` → `flipToXot` → `xot-pfs` 引擎。

---

## 5. 协议层（`recon/api.md`，468 行）

- **去重 95 条 URL、主进程实际可发起 ≈ 52 条 endpoint**（up366 43 + 外部 5 + 私有云 4），24 个 host；死配置/无调用点条目见 `api.md §1.15`。
- 传输层是 `Electron net.request` 自研封装，不是 axios；POST 默认 `x-www-form-urlencoded`。
- 7 个固定头：`user-agent: PC-Up366-Student 6.11.0`、`x-requested-with: PC`、`X-App-Name: student-pc`、`clientId: <MachineCode>`、`u3r: <每请求随机 UUID>`、`u3t: <TGT>`、`Authorization: <恒空串，全 bundle 无赋值>`。
- **没有逐请求签名**。只有登录/短信/成绩提交等敏感接口带 `ut`：`md5(ts).slice(0,10) + md5(payload + 关键词 + md5(ts)) + md5(ts).slice(10)`。
  `ts = md5(String(Date.now()))` 不可逆 ⇒ 服务端无法还原时间、**无法施加时效窗口**。
- 唯一真 HMAC：文件元信息 `HMAC-SHA1(key="au66ssoI3nGr6DxCHk8KcpVsQ1wYNQId", msg="GETOBJECT"+app+fileIds)`。
- 鉴权 = **登录 Cookie 为主**（`UP366-C`/`CASTGC`，Domain=`.up366.cn`；实测仅 Cookie 可访问各业务 host，仅 `u3t` 返回 -21）；`u3t`(TGT) 仅在 Cookie 缺失/过期时用于向 sso `/v1/serviceTicket` 换一次性 ST，以 `?ticket=` 重放一次。
- 登录态：token 落地在 `{saveDir}/setup.cfg` 的 **SQLCipher** 库，`PRAGMA key` 硬编码 `{3E2A243F-7B8B-45C8-9E7D-885A891820A7}`；TGT/ST 仅内存。
- **无 WebSocket / 无 SSE / 无长连接**，全部请求-响应 + 定时轮询（minicloud status 10s、二维码状态由渲染层轮询）。
- 环境切换靠 Proxy 把字面量 `up366.cn` 替换为 `up366.com.cn` / `up366demo.cn` / `up366test.cn`。
- 错题本（`wrongnote-api`，10 个端点）的 H5 侧字段、鉴权链与 `questions/kill` 读/写纠正在 `recon/wrongnote-h5.md`（2026-11-05 补；同日用测试账号只读验证，写接口仅 dry-run）。

---

## 6. 安全结论（`recon/security.md`，655 行，7 高 / 7 中 / 4 低）

### 必须优先处理的三条

1. **凭据经桥接直接外泄**（高危）
   `Login.getUserInfo()` / `Login.getToken()` 是 RPC 方法，在 258 个白名单里且**不校验 senderFrame**。
   任意渲染进程（包括被打开的远端页面）一句 `window.Login.getUserInfo()` 就能拿到**明文账号密码**和 TGT，不必读盘、不必解 SQLCipher。
2. **渲染层零隔离**（高危）
   全部窗口 `contextIsolation:false` + `webSecurity:false` + `webviewTag:true`，preload 随 `nodeIntegrationInSubFrames` 注入所有子框架，
   再叠加 `setWindowOpenHandler` 无条件 `allow` 与 `will-attach-webview` 强制覆盖 preload —— 任意远程页面直接继承上述 258 个方法。
3. **`local://` 任意文件读**（高危）
   `local://path/C:/Users/...` 直接返回去首斜杠的绝对路径；`resources/flipbooks/cache` 分支 `decodeURI` 后 `join`，无 realpath/前缀校验（`..` 穿越）。
   scheme 同时开了 `bypassCSP` + `corsEnabled`。

### 其余

- 更新链：包 URL 的 http→https「升级」是**丢弃返回值的空操作**；仅校验响应体自带 MD5；无降级比较；更新器管理员权限运行。
- 插件目录 `%saveDir%\plugins`（用户可写）无签名地被 `require` / `vm.runInThisContext`。
  ⚠ **更正**：实测**不是启动即执行**——`init()` 只登记 `config.json`，模块在 `load(name)` 时才加载，需要一次触发。
- `#/dialog` 用 `innerHTML` 渲染主进程下发的 `opts.title/content`（无 CSP）→ 服务端可控文案可变成注入。
- 离线播放器 `loadScript(resPath + "page1.js")` + `eval(xhr.responseText)`，书籍包无签名 → 渲染层独立代码执行路径。
- `ignoreCertificate` 开关可全局关 TLS 校验；`request/httpRequest` 是渲染进程的主进程 HTTP 代理（SSRF/任意头）。
- Electron 22.3.27 / Chromium 108 已过支持期；"主动加载远端页面 + 无沙箱" 使引擎内存破坏类问题等价本机用户代码执行。

---

## 7. 前端控制能力（`harness/`，已实测）

三档，成本递增：

| 档位 | 手段 | 能力 | 状态 |
|---|---|---|---|
| A. 页面级 | `.u3enc` 重新加密替换页面 + 注入 `_dev/hook.js` | 完整 DOM/CSS、fetch/XHR/WS 拦截与 mock | **已跑通** |
| B. E2E 驱动 | 页面 hook 轮询本地 `127.0.0.1:7801` 控制服务，按 `location.href` 路由到具体窗口/iframe | `eval` / `click` / `type` / `wait` / `dom`，可驱动登录 | **已跑通**（实测登录成功） |
| C. 主进程 Node | 插件目录 `require` 或挖 IPC 触发点 | `webContents.executeJavaScript`、真实 CDP、窗口/原生能力 | **未做**（需触发链） |

关键坑：窗口最小化时 `document.visibilityState === "hidden"`，`innerText` 为空、懒加载组件不初始化，看起来像"页面没渲染"。
`harness/dev-launch.ps1 -Offscreen` 把窗口挪到屏幕外并 `ShowWindow` 后 `visibilityState` 变 `visible`，页面完整。

---

## 8. 评测引擎（`recon/ases.md`）

- 进程模型：渲染 JS → `MessagePort` → Electron `utilityProcess` → `libases.node`(N-API) → `lsx-ases.dll` / `api-wavchecker.dll`。
- `libases.node` 的 **PE 导出表为 0**（`DataDirectory[0]=0/0`），静态链接；JS 侧可见 2 类 23 成员（AsesEngine 16 + WavChecker 7）。
- `model/score/` 15 个目录，13 个带 `score-basic-feature.json`；模型资产 50 个 `.model`（LightGBM + libsvm）、4 套 Kaldi、1 个 ONNX（NISQA）。
- **采集 → 解码 → 声学打分 → 特征 → 模型 → 分数，全部本地可复现**；只有 `net` 文件与 `/feedback` 上报依赖服务端。

---

## 9. 未解 / 明确不做

| 项 | 状态 |
|---|---|
| asar 头部索引加密 | 未解（原生侧另一把密钥）。**判定不做**，无增量价值 |
| `resources/res/6tmxa2VTDs.txt`（16B） | 全树 0 引用，判定孤儿，关闭 |
| 服务端风控（`/front/user/verify/send` 的 IP/频率/图形验证码） | **未验证**，报告中标为推断。不做——验证要真发短信，越界 |
| 主进程插件触发链 | 未挖，需要时可继续 |
| 付费/题库内容 | 未做，也不做 |

---

## 10. 一句话风险评级

加密层：**形同虚设**（密钥硬编码、可逆、白名单只覆盖 5 个后缀）。
架构层：**高危**（渲染层零隔离 + 258 个无来源校验的桥接方法 + 任意文件读写 + 任意 URL 进特权窗口）。
对用户最直接的一条：**明文口令既落盘、又能被任意渲染进程一句 JS 取走**。
