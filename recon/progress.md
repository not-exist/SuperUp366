# 进展记录

## 2026-11-01 P0/P1/P2 进展

### 已达成：app.u3a 与 .u3enc 全部透明解密

**方法**：Electron fuse `RunAsNode=1` 可用，
`ELECTRON_RUN_AS_NODE=1 up366.exe -e "$(cat tools/dump_archive.js)"`
即得到一个 Node 16 进程，但它用的是**厂商打过补丁的 fs**：
`fs.readFileSync(".../resources/res/index.html")` 直接返回 `<!DOCTYPE html>`，
`fs.readFileSync(".../app.u3a/package.json")` 直接返回归档内明文。

证据：
- `fs.statSync(".../app.u3a")` → isDirectory=true（asar 虚拟目录）
- `fs.readdirSync(".../app.u3a")` → `["dist","package.json"]`
- 归档内条目 `dist/main/index.js` = 3,901,687 B，明文开头 `"use strict";"use strict";`
- `res/index.html.u3enc` 读出的前 40 字节 = `<!DOCTYPE html>\n<html lang="en">`

**产物**
| 路径 | 内容 |
|---|---|
| `extract/app.u3a/` | 14 文件 17.5 MB（main 进程、preload、utils/libup366、loading） |
| `extract/res/` | 46 文件（Vite 构建的前端 + xotFullScreen + static/login、micTest） |
| `extract/plugins/ases/` | 插件 JSON 配置与特征表 |

**噪声点（后续用得上）**
- `fs.readFileSync('x.js')` 对普通磁盘文件会报 `ENOENT ... 'x.js.u3enc'`：补丁把 `x.js` 重定向到 `x.js.u3enc`，找不到就抛该错误。所以脚本一律用 `-e "$(cat file)"` 传入，不要 require 本地 js。
- 归档内 `dist/res/loading/index.html` 读取报 ENOENT（唯一失败项）。

### 运行时通道矩阵（recon/runtime-access.md）
| 通道 | 结果 |
|---|---|
| `--inspect=9229` | 被禁 |
| `--remote-debugging-port=9222` | 被禁 |
| `NODE_OPTIONS=--require` | 被禁 |
| `ELECTRON_RUN_AS_NODE=1` | **可用** |

### 启动期观测
- 主进程给渲染进程下发：`--no-sandbox --no-zygote --node-integration-in-worker`，
  `--standard-schemes=local,redirect --secure-schemes=local,redirect --bypasscsp-schemes=local --cors-schemes=local,redirect`
  → 自定义协议 `local://` 同时开了 bypassCSP 与 CORS，且渲染进程关闭沙箱（安全项，待 P5 展开）。
- 启动 8s 内建立外联：`36.151.61.44`。
- `%APPDATA%\up366\config.json` 仅 `{"saveDir":"C:\\Up366StudentFiles"}`；crashpad 数据库指向 `C:\Up366StudentFiles\logs\crash`。

## 2026-11-01 追加：前端控制 harness 已打通

- `.u3enc` 算法确认为 **AES-128-CBC + 前置 16 字节 IV + PKCS#7**，密钥 `QJBNiBmV55PDrewyne3GsA==`（base64，硬编码于 up366.exe 偏移 128158069）。
  实现：`tools/u3enc.js`（加解密双向，实测往返一致）。
- **可以重新加密并替换前端资源**：把 `resources/res/index.html.u3enc` 换成自己的页面后，`work/client` 副本正常启动，无完整性校验拦截。
- 渲染窗口未开 nodeIntegration（页面内 `require` 未定义），但 `contextIsolation:false` / `webSecurity:false` / `webviewTag:true` 仍然成立。
- 已交付：`harness/dev-prepare.ps1` + `harness/dev-launch.ps1` + `harness/payload/_dev/hook.js`，
  实测启动后窗口标题变为 `U3DEV-OK`，即注入脚本在真实渲染进程里执行。
- 插件完整性校验存在但只覆盖 `resources/plugins/ases/ia32/win32`（126 个文件，启动日志 "校验评分引擎文件完整性成功"），不覆盖 `resources/res`。

## 2026-11-01 收口：四份专项报告 + 两条结论更正

### 产出
| 文件 | 内容 | 关键数字 |
|---|---|---|
| recon/api.md | 主进程网络层 | 56 个真实 endpoint / 24 个 host；无逐请求签名；u3t(TGT)+ST 换票；ut=md5 拼接可离线构造；文件元信息 HMAC-SHA1（key 硬编码）；token 存 SQLCipher `setup.cfg` Config 表；无 WebSocket/SSE |
| recon/security.md | 安全评估 | 6 高危：桥接 258 方法无来源校验、contextIsolation:false + webSecurity:false + 全局 --no-sandbox、setWindowOpenHandler 全放行、`local://path/C:/` 任意读、更新链 http→https 空操作、插件目录加载 |
| recon/frontend.md | 渲染层 | 6 入口 / 12 路由 / 25 条服务端 API；Vite SPA 自身 0 条直连，业务全走 preload 桥 |
| recon/ases.md | 语音评测 | libases.node 导出表 0 个（静态链接）；JS 侧 2 类 23 成员；13 个题型带特征表 |

### 更正 1：app.u3a 主体不是明文，也不是 AES
三轮修正后的最终结论：`app.u3a` 就是改名的 Electron asar，条目**按后缀选择性加密**——
`.xml/.js/.html/.htm/.css` 用与 `.u3enc` **完全相同的密码原语和同一把密钥**（`40904d88…`），其余后缀明文，
所以明文/密文交替出现。此前 chi²=18,616 是混合样本造成的假象。
实测锚点：4100 明文 package.json、8023 明文 HTML、5328..5344 为 IV、5344 起解出 lib-up366 模块（~2.7KB）。
**唯一未解是 asar 头部索引**（前 8 字节也是密文，u32le@0 应为 4 实为 1,492,020,612），判为原生侧另一把密钥，需逆 `archive.cc` 常量。

**不需要再投入**：厂商 fs 层读取时自动解密，`tools/dump_archive.js` 已全量取出 14 文件 17.5 MB。
两条防错警告：CBC 流内任意块边界都能"解出可读文本"（实测 5328 后有 165 个假阳性），不要按对齐枚举条目起点；也不要按明文 carve（明文/密文交替）。

### 更正 2：插件目录不是"启动即 RCE"
security.md 把 `%saveDir%\plugins` 与安装目录 plugins 的加载判为持久化 RCE。
实测（work/client/resources/plugins/u3dev/{config.json,main.js}，带写盘副作用）启动后 **未产生 marker**：
`init()` 只做 `loadPluginInfo`（读 config.json 登记），模块本体由 `nQe()` 在 `load(name)` 时才 require/u3enc 执行。
所以准确表述是：**需要一次触发的代码执行**（能被 IPC 拉起的插件名），不是开机自启。危害等级应下调，触发链未验证。

### 6tmxa2VTDs.txt
全树 0 命中（文件名与 16 字节内容都不被任何随包产物引用），标记为无关联，关闭。

### 偏移口径校正（frontend 复核）
frontend.md 全文 48 处 `@偏移` 已统一为「UTF-8 解码后字符下标」并逐条回读校验，结论未变。
其中若干处原值确实错误（userInfo 明文密码 250000→263810；eval 响应体 873700→885072；顶象 CDN 241000→266890；goods 272100/272200→271699/271764 且顺序颠倒等）。

**已修正**：security.md §9.1-b 原引用的 @192433 其实是 password 输入框的 `on:clear` 处理器，不是回填点。
正确锚点（`static/login/login.bdb7e7c2.js`，字符下标）：@263810 `saveData("userInfo",{userName,password})` →
@264933 `JSON.parse(Login.loadData("userInfo"))` → @265002 `this.form.password=n.password`。
security.md 已单点更新至 45,430 B，全文 192433 出现 0 次，结论不变。

## 2026-11-05 错题本（wrongnote-api）H5 实证

- 新增 `recon/wrongnote-h5.md`：从 `student.up366.cn` H5 6.12.1 取全 10 个端点的 body/query/响应字段。
- **鉴权更正**：wrongnote-api 用 TGT → 一次性 ST → `?ticket=`；H5 在 PC 客户端内由 Electron 拦截 sso 换票
  （`preload/index.js@3640984` → `redirect://getServiceTicket` → PC TGT）。实测：无 ticket=`-21`、`ticket=INVALID`=`-22`、sso 无 TGT=`-23`。
- **读/写更正**：`/student/questions/kill` 是读接口（取待消灭题包），不是写；真正写接口是 `/student/question/submit`。
- `wrong_books` 的「全部/练习」是 H5 页面 UI 注入，不是服务端数据（`chunk-6ff8c6e6.js@12504`）。
- `finished_tasks`：PC `GET {bookId}`（@3617392）、H5 `POST {bookId}`（chunk-6b733b35.js@9507、chunk-39edd6e0.js@17926）；
  两端都不传 `courseId`；实测两种方法均返回 `-21` JSON（网关两条路由都存在）。
- `answer_detail`：PC 调用点 `ExportContinueWritingReport` @3768493（此前误判为死代码），参数 `{courseId,bookId,taskId,uid}`；
  H5 `POST /v2`（chunk-6c496ba5.js@892819）参数 `{uid,bookId,taskId}`；响应均为 `data.paperInfo` + `data.userAnswer`。

### 2026-11-05 有效账号只读 e2e

- 用测试学生账号跑通：`wrong_subjects` / `wrong_subject_stat` / `wrong_books` / `wrong_knowledge` /
  `wrong_courses` / `wrong_rank` / `wrong_question_show` / `wrong_killed_questions` / `wrong_kill_paper` /
  `finished_tasks` / `answer_detail` v1+v2；全部 `code=0`，结构与 `recon/wrongnote-h5.md` 一致。
- 鉴权矩阵：仅 Cookie=`0`，仅 `?ticket=`=`0`，仅 `u3t`=`-21`，无凭证=`-21` → Cookie 或 ST 任一即可，`u3t` 单独无效。
- 分页：body-only / query-only / 两者同时三者等价（knowledge、rank 都验证）。
- `finished_tasks`：GET 与 POST 同 `bookId` 返回完全相同的行；`courseId` 被忽略。
- `answer_detail`：v1/v2 都需要 `{courseId,uid,bookId,taskId}`；`taskId` 取 `contentId`；v2 缺 `courseId` 报 `param_missing`。
- 写接口 `question/submit` 只做 dry-run，未真实提交（会改动账号数据）。
