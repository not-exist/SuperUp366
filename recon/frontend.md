# up366 渲染层（前端）测绘

> 范围：`H:/project/up366/extract/res`（已解密产物）+ `extract/xotFullScreen-src`（由 .js.map 还原的源码）。
> 未读取主进程代码（`extract/app.u3a`），凡涉及主进程行为的结论都标注为「需主进程侧确认」。
> 所有 bundle 为压缩单行，行号无意义，证据以「文件@偏移」标注。
> **偏移口径**：UTF-8 解码后的字符下标（非磁盘字节；含中文的 login.bdb7e7c2.js 两者相差约 6 万）。复核方式：`open(p,encoding='utf-8').read()[off:off+80]`。采集日期 2026-11-01。

---

## 0. 结论摘要

| 项 | 值 |
|---|---|
| HTML 入口 | **6 个**（另有 1 个 `resources/res/loading/index.html` 存在于原始安装目录但未提取成功） |
| Vite SPA 路由 | **12 条**，hash 模式 |
| 渲染层直连服务端 API | **25 条**（全部集中在 2 个老构建产物里；Vite SPA 自身 0 条） |
| 第三方 / 外链 | 8 处（验证码 CDN、微信/QQ OAuth、CNZZ 统计、student 站跳转、msd 站） |
| 本地存储 key | 11 个（Vite SPA 全走 `AppInfo.getMemoryData`，不用 localStorage） |
| IndexedDB | 无（仅 lokijs 字符串引用，未使用） |

核心结构：**Vite 壳（Vue3 + Element Plus，全部走 preload 暴露的全局对象）+ 三个遗留页（Vue2 + 自家 u3- 组件库，直连 HTTP）+ 一个离线全屏讲解播放器（Vue2 + jQuery + xot-pfs 引擎）**。

---

## 1. 入口与路由表

### 1.1 HTML 入口 → 加载的 chunk

| 入口 | 加载的 js | 加载的 css | 依据 |
|---|---|---|---|
| `res/index.html` | `assets/index.5da9b73f.js` | `assets/index.25db731a.css` | index.html 内 `<script type=module>` / `<link rel=stylesheet>` |
| `res/login.html` | `static/common/chunk-vue.7b205dfa.js` + `static/login/login.bdb7e7c2.js` | `static/login/login.e37b6b85.css` | login.html 底部两个 `<script src>` |
| `res/micTest.html` | chunk-vue + `static/micTest/micTest.169f0101.js` | `static/micTest/micTest.e719a567.css` | micTest.html |
| `res/network.html` | chunk-vue + `static/network/network.34b9a2f9.js` | `static/network/network.97394bb5.css` | network.html |
| `res/dialog.html` | 无外部 js（内联脚本 765 B） | 无 | dialog.html 全文 |
| `res/xotFullScreen/index.html` | `js/chunk-vendors.891aa818.js` + `js/app.1ae30858.js` | `css/app.8252afd4.css` | xotFullScreen/index.html |
| `res/loading/index.html` | **未提取**（progress.md 记录 asar 内 ENOENT） | — | recon/progress.md |

`res/6tmxa2VTDs.txt`：32 位 hex 单行（`39dc2d2d01becd9258b706e516542396`），无任何文件引用，用途未知（疑似许可/校验用盐）。

### 1.2 `index.html` 的 Vue Router 路由表（12 条）

证据：`assets/index.5da9b73f.js` @1000322 处 `const rme=[{path:"/",component:()=>yo(()=>import("./main.3da643cd.js"),[…])}…]`；@1001948 处 `tR=iF({history:DB(),routes:rme})`。

hash 模式确认：`function DB(e){return e=location.host?e||location.pathname+location.search:"",e.includes("#")||(e+="#"),LB(e)}`（@100307）→ 等价 `createWebHashHistory`，实际 URL 形如 `index.html#/login`。

| # | path | 组件 chunk | 页面用途 | 依据 |
|---|---|---|---|---|
| 1 | `/` | main.3da643cd.js | 主窗口主页：标题栏（最小化/最大化/关闭/双击还原）+ 顶栏（我的教学班 / 错题&生词 / 消息 / 头像 / 私有云状态）+ 内嵌原生主视图 | `Login.on("loginStateChanged")`；`CurrentWindow.showMainView(n)`；私有云图标 title：私有云未注册/未连接/已连接/数据上传成功/正在上传/上传失败 |
| 2 | `/login` | login.b1714e53.js | 登录窗口壳：**iframe 内嵌 login.html**，并用 `window.frames.iframeMap` 反向调用 iframe 里的 `onLogin/onQrLogin/showLoading/isMainWindow/ReLogin` | `getPageUrl:"login.html"`；`<iframe name="iframeMap">`；`executeScript(e,i){if(window.frames.iframeMap&&window.frames.iframeMap[e])return window.frames.iframeMap[e](...i)}` |
| 3 | `/loading` | loading.107aab17.js | 启动加载进度页（「请耐心等待，小天正在奋力加载…」），进度来自 `CurrentWindow.on("packageLoading",{msg,progress})` | loading.107aab17.js@0 |
| 4 | `/menu` | menu.a8bf9691.js | 悬浮侧边菜单（设置/检查更新/运行环境检测/关于/个人信息/我的课本/问题反馈/录音检测/退出登录 + 课程列表 + 搜索） | menu.a8bf9691.js@4226 起 methods |
| 5 | `/toolbar` | toolbar.2a4fcc89.js | 主窗口工具条（缩放-/缩放+/重置、主页、最小化、折叠贴边拖动） | toolbar.2a4fcc89.js@0 |
| 6 | `/dialog` | index.a16a3d07.js | 通用对话框窗口：设置（代理/私有云/缓存目录/自动更新/评分日志）、关于、下载、导出课本、客户端升级、口语测评反馈、二次确认 | `components:{download,about,setup,exportbook,appupdate,closeConfirm,feedback}`；`CurrentWindow.on("dialog")` 按 `opts.type` 分发 |
| 7 | `/offlinebar` | index.2ffed8d6.js | 离线模式悬浮条（logo +「点击 登录 体验更多功能」），按内容尺寸回报 `setOfflineBarSize` | index.2ffed8d6.js 全文 |
| 8 | `/separate` | index.b561b386.js | 独立小窗容器：`?url=` 解码后塞进 `<iframe name="iframeMap">`，并暴露 `window.loadUrl` 供子页切页 | index.b561b386.js 全文 |
| 9 | `/separate/toolbar` | toolbar.c698d5fd.js | 独立小窗标题条（胶囊展开/关闭）→ `operateSeparateWindow({opt:"show"\|"close"})` | toolbar.c698d5fd.js 全文 |
| 10 | `/pdf/upcode` | upcode.ee947d5b.js | 打印页：课程名 + 每个学生一张 `upcode` 二维码图（`stuList[].uid/upcode/userName`），可选回执页 | `NetBrowser.getUpcodeData()`；`s("img",{src:o.upcode})` |
| 11 | `/pdf/dictation-report` | dictation-report.a8fb8df7.js | 听写报告打印页：`reportList` 全是图片 URL，逐张渲染 | `NetBrowser.getDictationReportData()`；`{src:a}` |
| 12 | `/pdf/writing-report` | writing-report.8603a593.js | 写作报告打印页：满分/得分/「出彩表达：」 | `NetBrowser.getWritingReportData()`；中文串「分 / 出彩表达： / 满分」 |

- 路由守卫（@1001981）：`tR.beforeEach((e,t,n)=>{!window.Login.State&&e.path==="/"?n({path:"/login"}):n()})`。
- `/pdf/*` 三页同一套模式：数据整包由主进程塞进 `NetBrowser.get*Data()`，页面只渲染图片/文本，`mounted` 里轮询 `document.querySelectorAll("img")` 全部 complete 后 `CurrentWindow.emit("viewReady")`。**推断**（未在主进程侧验证）：主进程隐藏窗口加载这三页 → 等 viewReady → printToPDF/截图导出。证据：全仓只有这三个 chunk 出现 `viewReady`（grep -l 结果）。

### 1.3 Vite 应用壳（index.5da9b73f.js，1,002,457 B）

- Vue 3 + vue-router 4（hash）+ vuex 4（`state:{user:AppInfo.CurrentUser,screen:{width,height}},mutations:{login}`）+ Element Plus 全量 + axios + lodash + vue-devtools hook。
- 全局属性@1002169：`Eb.config.globalProperties.$filter={bytesToSize:Nve}`、`Eb.config.globalProperties.$http=ZD`。
- **`$http` 只有定义、无任何调用**（全 bundle `$http` 计数 = 1）→ Vite 侧业务数据 100% 走 preload 全局对象。

---

## 2. 渲染层直连的接口

结论：**Vite SPA 一行 HTTP 都不发**。需先排除的噪声：

- `assets/index.5da9b73f.js` 的 `fetch(` 1 处是 Vite `modulepreload` polyfill（@672）；`XMLHttpRequest` 6 处来自 axios 与 Element Plus upload 组件；64 个 http(s) 字面量全是 element-plus/lodash/vue 文档注释与 SVG 命名空间。
- `static/micTest`、`static/network` 除库代码外无业务 URL（micTest 0 条中文串只有图片；network 只有「网络怎么可以这么差 / 点击重试」）。

### 2.1 遗留登录页 `static/login/login.bdb7e7c2.js`（axios 直连，15 条）

基址常量（@183800）：

```
xt="https",
St=xt+"://course-api.up366test.cn",    // 课程
Ct=xt+"://studytask-api.up366test.cn",  // 学习任务
Ot=xt+"://book-api.up366test.cn",       // 课本
Et=xt+"://sso.up366test.cn",
jt=xt+"://user-api.up366test.cn",       // 用户
kt=xt+"://setup-api.up366test.cn",      // 设置/版本/配置
Pt=xt+"://logs-api.up366test.cn",       // 日志
It=xt+"://mall-api.up366test.cn",       // 商城
Mt=xt+"://student.up366test.cn", Nt="https://student-gray.up366test.cn"
```

> **两点必须注意**：
> 1) 该产物把基址**硬编码为 `.up366test.cn`（测试环境）**。主进程只对 `redirect://getServiceTicket` 做 `onBeforeRequest` 拦截，**不重写 host**（见 recon/api.md），因此页面上真实发出的请求就是打到测试域；生产域名只出现在 OAuth 跳转与《服务合同》文本里。
> 2) 这 15 条来自同一份「网页版登录应用」。**客户端模式下**登录/验证码/用户信息/已购课本都走 `window.Login` 桥（`Login.login`、`sendVerifyCode`、`NetBrowser.getUserInfo`、`getBuybook`），所以下表多是 web 兜底路径；#1/#2 只在用户点「下载客户端」时触发（`downloadPC` / `downloadTeacherPC`）。

| # | 方法 | URL | 说明 | 证据 |
|---|---|---|---|---|
| 1 | POST | setup-api `/front/appversion/latestVersion?modelName=PC-UP366-V2-EXE` | 下载学生端 PC（按钮 `downloadPC` → `window.open(url)`) | `de=` @187365 |
| 2 | POST | setup-api `/front/appversion/latestVersion?modelName=PC-UP366-TEACHER` | 下载教师端 PC（`downloadTeacherPC`） | `pe=` @187461 |
| 3 | POST | setup-api `/client/up366/token` | 取 SSO token（`data.up366token`） | `he=` @187558 |
| 4 | POST | setup-api `/front/html/configs` | 取前端配置（外链地址表） | `ve=` @187616 |
| 5 | POST | logs-api `/front/uploadmsg` | 日志上报 | `le=` @187299 |
| 6 | POST | user-api `/front/user/verify/send` | 短信验证码（web 路径；客户端走 `Login.sendVerifyCode`） | `ge=` @187920 |
| 7 | POST | user-api `/client/user/currentuser` | 当前用户，**有 `NetBrowser.getUserInfo()` 前置短路** | `_e=` @187982 |
| 8 | POST | user-api `/client/user/class/info` | 班级信息 | `we=` @188176 |
| 9 | POST | course-api `/client/course/list-of-student/v2` | 学生课程列表 v2 | `kr=` @267889 |
| 10 | POST | course-api `/client/student/course/book/list` | 课程课本列表 | `Pr=` @267961 |
| 11 | POST | course-api `/client/student/course/list` | 课程列表 | `Ir=` @268032 |
| 12 | POST | studytask-api `/client/student/course/stat/list` | 课程学习统计 | `Rr=` @268109 |
| 13 | POST | book-api `/client/books/buy-book` | 已购课本，**有 `NetBrowser.getBuybook()` 前置短路** | @271986 |
| 14 | POST | mall-api `/front/goods/category/list` | 商品分类 | @271699 |
| 15 | POST | mall-api `/front/goods/list` | 商品列表 | @271764 |

公共中间件（`Zt(Bt.a,se,ae)`，@187261）：

- 统一 `timeout` 10 s（`ae()` 改为 30 s）、`withCredentials`、`X-Requested-With: XMLHttpRequest`；
- URL 命中 `/user-api.` 或 `/sso.` 时追加头 `Custom-Agent: student-web-6.5.5`；
- 响应码协议：0 / -24 成功；-21 / -22 → 取 `serverTicketUrl` 换票后**重放原请求**；-23 / -25 未登录；-4 无权限。

### 2.2 离线全屏播放器 `xotFullScreen/js/app.1ae30858.js`（10 条 + 1 外链）

`serverconfig` 模块（@883376）按 `document.domain` 推域名：含 `up366test.` → up366test；含 `up366demo.` / `localhost` → up366demo；否则 up366。再拼 `https://<svc>-api.<platform>.cn`。

| # | 方法 | URL | 用途 |
|---|---|---|---|
| 16 | POST | `pigai-api.{p}.cn/client/correct/async` | 批改（异步） |
| 17 | POST | `pigai-api.{p}.cn/client/result` | 取批改结果 |
| 18 | POST | `pigai-api.{p}.cn/client/batch/result` | 批量取批改结果 |
| 19 | POST | `pigai-api.{p}.cn/client/correct/async/v2` | 批量提交（batchCommit） |
| 20 | POST | `pigai-api.{p}.cn/client/async/correct/gecscore` | 应用文写作提交 |
| 21 | POST | `setup-api.{p}.cn/client/userfeedback/write/save` | 反馈「批改不准」 |
| 22 | POST | `self-api.{p}.cn/selfstudy/similar/questions` | 同类题抽题 |
| 23 | POST | `self-api.{p}.cn/selfstudy/similar/judge` | 判断有无同类题 |
| 24 | POST | `qlib-api.{p}.cn/client/fileinfo/sign` | 取 ppt/pdf 附件签名（appKey=qlib） |
| 25 | GET | `fs-v2.{p}.cn/fileinfo/{fileId}` | 取附件转换状态 |

外链页：`https://msd.{p}.cn/#/`。另含硬编码样例媒体 `https://qlib-nginx.up366test.cn/qlib-files/papers/2019/1010/…/media/1-ZC.mp3`。

### 2.3 第三方 / 外链（8 处）

| # | 目标 | 触发条件 | 证据 |
|---|---|---|---|
| 26 | `https://cdn.dingxiang-inc.com/ctu-group/captcha-ui/index.js`（顶象滑块） | `if(!window.AppInfo)` → **仅浏览器打开时注入，客户端内不注入** | login@266890 |
| 27 | `https://open.weixin.qq.com/connect/qrconnect?…` | `window.location.href=` 整页跳转（浏览器路径） | @262298 |
| 28 | `https://graph.qq.com/oauth2.0/authorize?…` | 同上 | @261911 |
| 29 | `s9.cnzz.com/z_stat.php?id=1279092325` | 仅当 `location.origin` 含 `up366.cn` | @190377 |
| 30 | `https://student.up366{platForm}.cn/#/login/sno`、`/#/account/{register,guide,forgetPassword}` | 点注册/找回密码/账号指引时**把渲染窗口导航到远端站** | @256466 起 5 处 |
| 31 | `http://www.up366test.com/{wechat,qq}-redirect.html` | OAuth 回调地址 | @183900 |
| 32 | `https://msd.{p}.cn/#/` | 同 2.2 | — |
| 33 | 主进程下发的 `getConfigUrl(key)` / `showMainView(key)` 外链表 | 已观测 key 5 个：`myInfoUrl` `myBookUrl` `feedbackUrl` `myMessageUrl` `wrongNote`；表来源于 #4 `/front/html/configs` | menu.a8bf9691.js `goToUrl`/`showDialogUrl`；main.3da643cd.js `goToUrl` |

> #33 是渲染层「可被服务端牵引」的关键：**配置表在服务端**，客户端只拿 key 换 URL 并加载。需主进程侧确认 key→URL 解析与缓存。

---

## 3. 本地存储 key 清单

Vite SPA 的「存储」全部是 preload 暴露的进程内存储（`AppInfo.getMemoryData/setMemoryData`），**不落 localStorage**；`index.5da9b73f.js` 里 2 处 `localStorage` 属 vue-devtools 的 `__vue-devtools-plugin-settings__*`。

遗留页有统一存储垫片（login / micTest 各一份，代码相同）：

```
// set:  (b(e)||y(e)) && (e=JSON.stringify(e))
NetBrowser.saveData ? NetBrowser.saveData(t,e) : localStorage.setItem(t,e)
// get
e = NetBrowser.loadData ? NetBrowser.loadData(t) : localStorage.getItem(t)
// remove
NetBrowser.clearData ? NetBrowser.clearData(t) : localStorage.removeItem(t)
```

即：**客户端内 localStorage 是空壳，数据走 `NetBrowser.*Data` 到主进程**；浏览器直开时才落 DOM Storage。

| key | 后端 | 内容 / 含义 | 证据 |
|---|---|---|---|
| `USER_INFO` | bridge 垫片 | 登录后用户对象；退出时 `localStorage.removeItem+clearData` 双清 | login@184536、@188238（`xe="USER_INFO"`） |
| `userInfo` | bridge（`Login.loadData/saveData`） | **记住密码**：`{userName,password}` **明文 JSON**（勾选 remember 才写，否则写 `{}`） | @263810；启动时回填 `n.password` |
| `noGoIndex` | localStorage（直写，未走垫片） | `'true'` = 本次不要自动跳主页（无权限/未登录后置位，回登录页置 `'false'`） | @186810、@265758 |
| `IsLatestLogin` | localStorage | 最近一次登录成功标记（值 `true`，被 setItem 强转字符串） | @261296 |
| `hasSaveUserData` | **sessionStorage** | 已缓存用户数据标记 `'true'` | @188336 |
| `loglevel` | localStorage + cookie 双写 | 日志级别（silent/trace/debug/info/warn/error） | @285588 |
| `showFirstLoginDialog` | bridge 垫片 | 首次登录弹窗只弹一次 | @265527 |
| `checkMobile` | bridge 垫片 | 手机号校验通过标记（值 1，与 `IsLatestLogin` 同一次回调里写） | @261279 |
| `book_extend_data` | bridge 垫片 | 课本扩展数据 `{bookId:{lastStudyTime}}`（暴露 `window.getExtendBookData`） | @190729 |
| `sessionData` | bridge（直调 `NetBrowser.loadData`） | 会话级 UI 记忆，已见字段 `closeMinicloudTips` | @254301 |
| `u3h` | **cookie**（domain `.up366login.cn`，expires=-1 即删除） | 灰度分流标记（student-gray / student 互斥选择） | @184536 `qt`；`Vt=()=>!!cookie("u3h")` |

另：`window.setUser/setUserData/stuSetUser/setExtendBookData/getExtendBookData/onLogin/onQrLogin` 是**页面暴露给主进程 `executeJavaScript` 的全局函数**（@190064 起），属「主进程 → 页面」的反向通道，不是存储。

---

## 4. xotFullScreen 离线逻辑

### 4.1 事实：打包进去的是**离线版**播放器

`js/app.1ae30858.js` 全文只有 **1 个 webpack 入口** `// CONCATENATED MODULE: ./src/main-offline.js`（@26974），且只有 1 处 `.$mount(".root")`（@27211）：

```
new Vue({ render: h => h(OfflinePfs) }).$mount(".root")
```

（在线全屏讲解不在资源里，应由远端 CDN 页在 `showMainView` 的 webview 内加载；xot-pfs 引擎 `preview-fullscreen` + `U3CompoLib` 是同一份，因此 bundle 内仍带在线接口 #16–#25。）

### 4.2 离线数据从哪来 / 什么格式

链路（`src/view/offline/offline-pfs*.vue`，还原源码 1.5 KB）：

1. **取资源根目录**：`const resPath = JSON.parse(NetBrowser.getPageParams()).resPath` —— 主进程通过页面参数传入**本地绝对路径前缀**（末尾带 `/`）。
2. **动态加载题目脚本**：`this.loadScript(resPath + "page1.js", cb)` —— `document.createElement("script")` + `document.body.appendChild`，脚本执行完产生**全局变量 `pageConfig`**。
3. **协议转换**：`flipToXot(pageConfig, resPath)`（`src/utils/flipToXot.js`，源码完整可读）。
4. **渲染**：`this.$refs.pfs.init({ paperObj: questObj, isLine: true })` → `xotPfs.init(pfsParams)`（`pfs.pfs`，来自 `lib/xot/xot-pfs/build/preview-fullscreen`）。`isLine:true` 即离线模式，引擎内 `if(isLine && NetBrowser.backToStudy) NetBrowser.backToStudy()`。

**page1.js 数据格式**（由 `flipToXot` 反推，snake_case）：

- 顶层是对象，值里递归挂题目节点；`dfs()` 遍历 `Object.values`，靠 `question_id` + `question_type` 判定节点，`question_type===99` 为**组合题**，其余为小题。
- 小题字段：`record_follow_read`(→attachment) `difficulty` `directions` `question_id`(→elementId) `extends` `knowledge` `options` `question_kind` `question_no` `question_score` `question_text` `questionTextCopy` `question_type` `ref_section_id` `ref_sub_id`；答案侧 `analysis` / `answer_text`。
- 媒体：节点有 `medias[]`；当 `medias[0].type==="media"` 时取 `record_follow_read.mode_list[0].media_file`（口语资源包），拼成 `resPath + "questions/" + question_id + "/" + media_file`，再合成一段 **HTML 片段塞回题干**：
  `<img media="audio" src="/lib/xoteditorv2/dialogs/xotspoken/images/media.jpg" url="<绝对路径>" has_audio="…" limit="-1" guid="<随机guid>">`
  → 题干是**富文本 HTML 字符串**，由 xot 编辑器内核按 `<img media=...>` 约定渲染成音频播放器。
- 跟读附件结构化：`attachmentChange()` 产出 `{baseUrl:resPath, paragraphs:[{pre,sentences:[{id,content,netfiles,keyno}]}], sentenceTimes:media.sentences, soundfile:media.media_file}`；`keyNo` 的句子包成 `<b>…</b>`。
- 组合题输出 `{id,isGroupText:'1',type:element_type,elementObj:{…subQuestions:[…]}}`；小题输出 `{elementObj:{…},id,type:'3'}`；最后整体 `.reverse()`。

### 4.3 能否本地完整渲染

**能，但有 3 个硬依赖**：

1. `page1.js` 必须存在于 `resPath` 下（课程资源目录，主进程管理，本次未验证磁盘布局）。
2. 媒体文件必须在 `resPath + questions/<qid>/` 下。
3. 题干里的 `src="/lib/xoteditorv2/..."` 是**相对站点根**的路径 → 依赖站点/协议把 `/lib/xoteditorv2/` 映射到真实资源（该目录不在 `extract/res`，只在 xot 源码 `src/lib` 里）。映射缺失时音频播放器图标 404，但题目文字仍渲染。

副作用（安全相关）：`page1.js` 是**用 `<script>` 执行的本地 JS**，等于向渲染进程投递任意代码；`flipToXot` 又把题干当 HTML 拼接（`questionText + "<img …>"`），题干内容不做转义。

---

## 5. 资源清单

`extract/res` 共 46 个文件。**注意：只有 `*.u3enc`（js/css/html）被解密落盘，全部图片/字体/gif 仍留在原安装目录 `resources/res/**`（见 inventory.csv），不在 extract 内**；下表按 bundle 引用关系还原用途。

### 5.1 `assets/`（Vite 构建，26 个 js/css）

| 文件 | 字节 | 用途 / 归属 |
|---|---|---|
| index.5da9b73f.js | 1,002,457 | 应用壳：Vue3 + Router(hash) + Vuex + Element Plus + axios + lodash |
| index.25db731a.css | 364,869 | Element Plus 全量样式 |
| main.3da643cd.js | 20,001 | 路由 `/` 主页组件（标题栏 + 顶栏 + 内嵌主视图）；内嵌 7 张 base64 图标 |
| main.2adfa312.css | 9,050 | 主页样式 |
| login.b1714e53.js | 6,087 | 路由 `/login`：iframe 壳 + 登录错误码枚举（0/-1/-2/-3/-4/-15…-10000） |
| login.f18ed938.css | 335 | 登录壳样式 |
| loading.107aab17.js | 1,129 | 路由 `/loading` 进度页 |
| loading.ea80e6c9.css | 1,340 | 进度页样式 |
| menu.a8bf9691.js | 8,373 | 路由 `/menu` 悬浮菜单 + 课程搜索（含 1 张 base64 二维码） |
| menu.6e9828e6.css | 931 | 菜单样式 |
| toolbar.2a4fcc89.js | 4,996 | 路由 `/toolbar` 工具条 + 贴边拖动 |
| toolbar.a2e6e2dd.css | 29,027 | 工具条样式（含图标雪碧） |
| toolbar.c698d5fd.js | 698 | 路由 `/separate/toolbar` 独立窗标题条 |
| toolbar.c72756e9.css | 6,523 | 独立窗标题条样式 |
| index.a16a3d07.js | 26,690 | 路由 `/dialog`：设置/关于/下载/导出课本/升级/反馈/确认 |
| index.ccced4f1.css | 24,048 | 对话框样式 |
| index.2ffed8d6.js | 997 | 路由 `/offlinebar` 离线条 |
| index.8587bc9f.css | 643 | 离线条样式 |
| index.b561b386.js | 833 | 路由 `/separate` iframe 容器 |
| index.b6011cae.css | 9,218 | 独立窗样式 |
| upcode.ee947d5b.js | 1,096 | 路由 `/pdf/upcode` 上机码打印 |
| upcode.8489476e.css | 628 | 同上 |
| dictation-report.a8fb8df7.js | 811 | 路由 `/pdf/dictation-report` 听写报告打印 |
| dictation-report.2dc23fc4.css | 353 | 同上 |
| writing-report.8603a593.js | 8,692 | 路由 `/pdf/writing-report` 写作报告打印（内嵌多张 base64 图） |
| writing-report.639a83e3.css | 4,530 | 同上 |

被引用但**不在 extract** 的图片（仍存在于原安装目录）：`backtip.d590b751.png` `logo.cbcbbcf4.png` `logo.fde8505d.png` `offline-logo.5f548380.jpg` `loading_xiaotian.24769e17.gif` `remark-icon.5a1ad829.png` `success.37acd5ad.png` `uploading.16983c52.gif`，另有 `background / close_* / enlarge / fold* / home_* / loginbg@2x / header@2x / min* / open* / reduce* / unfold*` 约 25 张（窗口按钮三态）。

### 5.2 `static/`（遗留 vue-cli 构建，7 个文件）

| 文件 | 字节 | 用途 |
|---|---|---|
| common/chunk-vue.7b205dfa.js | 80,150 | Vue **2** 运行时 + 公共 helper（三页共享） |
| login/login.bdb7e7c2.js | 598,829 | 登录/注册/找回密码全套（隐私政策与《技术服务合同》全文、短信/微信/QQ 登录、验证码、日志上报、15 条 API） |
| login/login.e37b6b85.css | — | 登录页样式 |
| micTest/micTest.169f0101.js | 598,533 | 录音检测页（0 条中文串，纯图片交互；`record-rating.gif` / `head-mic.png`） |
| micTest/micTest.e719a567.css | — | 同上 |
| network/network.34b9a2f9.js | 62,441 | 网络异常页（「网络怎么可以这么差 / 点击重试 / 点击返回」） |
| network/network.97394bb5.css | — | 同上 |

遗留页 UI 库是自家 `u3-` 组件（`U3Dialog/U3Captcha/U3Loading/U3Toaster`，login 里 71 处 `u3-` 类名），与 xot 源码 `src/lib/U3CompoLib-1.0.0` 同源。三页均**无 vue-router**，一 html 一页面。

### 5.3 `xotFullScreen/`（离线全屏讲解）

| 文件 | 字节 | 用途 |
|---|---|---|
| index.html | 735 | 挂载点 `.root` |
| js/app.1ae30858.js | 1,549,189 | 业务 + xot-pfs 引擎 + jquery/video.js/swiper/lodash/art-template/hammer/photoswipe/core-js；入口 `main-offline.js` |
| js/chunk-vendors.891aa818.js | 875,530 | Vue2 + jQuery + video.js + art-template + vuex + core-js |
| css/app.8252afd4.css | 334,507 | 全屏讲解 + U3CompoLib 主题 |
| js/*.map, css/*.css.map | 4.3 MB | sourcemap，**含 sourcesContent** → `extract/xotFullScreen-src/` 由此还原 |

`xotFullScreen-src` 还原出的业务源文件只有 9 个（`main-offline.js`、`components/XotPfs*.vue`、`view/offline/offline-pfs*.vue`、`utils/flipToXot.js`、`utils/flexible.js`），其余是 node_modules 与 `lib/` 第三方包（xot-pfs 引擎、U3CompoLib）。

### 5.4 其他

| 文件 | 说明 |
|---|---|
| `res/6tmxa2VTDs.txt` | 32 位 hex，无引用，用途未知 |
| `static/fonts/{element-icons.535877f5.woff, element-icons.732389de.ttf, u3-icons.b92e0136.ttf}` | 图标字体，未提取（在原安装目录） |
| `static/img/*`、`xotFullScreen/img/*`、`res/images/*`、`res/loading/*`、`res/favicon.ico` | 未提取（在原安装目录） |

---

## 6. 可注入面（为「前端控制能力」准备）

### 6.1 运行时前提（recon/progress.md，团队已验证）

- 渲染进程无 `nodeIntegration`（页面内 `require` 未定义），但 `contextIsolation:false`、`webSecurity:false`、`webviewTag:true`。
- 自定义协议 `local:` 同时注册 standard / secure / **bypassCSP** / CORS。
- `resources/res` 无完整性校验（只有 `plugins/ases` 有），**可替换 `.u3enc` 资源并重新加密**。
- 推论：`contextIsolation:false` 使页面 JS 与 preload 共享同一 `window` → **可直接改写 `window.Login/AppInfo/CurrentWindow/NetBrowser` 的方法与事件回调**，也全权操作 `document`。

### 6.2 页面级注入点

| 位置 | 形态 | 风险 / 用法 |
|---|---|---|
| `dialog.html` | `<webview id="frame">`，`src` 来自 `?url=`；`local://` 开头时对 `title`/`url` 做 `decodeURIComponent` | **任意 URL 载入 webview**（外网/本地文件/自定义协议）；`injectNav(title)` 由 preload 注入 |
| `#/separate?url=` | `<iframe name="iframeMap" src=decodeURIComponent(url)>` + 全局 `window.loadUrl(u)` | 子页与容器**同进程同源**（webSecurity:false），子页可 `parent.loadUrl()` 任意切页，也可直接摸 `parent.CurrentWindow` |
| `#/login` | iframe `login.html`，父页通过 `window.frames.iframeMap[fn](...args)` 调 iframe 全局函数（assets/login.b1714e53.js@5278） | 双向**未做来源校验**的函数调用通道；同源，拿到该 window 即可触发登录/改 URL |
| `#/dialog` | `innerHTML` 渲染 `opts.title` 与 `opts.content`（`s("div",{class:"title",innerHTML:e.opts.title})`@25462、`innerHTML:e.opts.content`@25766；事件订阅 `on("dialog",e=>…)`@23101，均 assets/index.a16a3d07.js） | **等价 v-html**：`opts` 由 `CurrentWindow.on("dialog",{event:"show",opts})` 从主进程传入；任何能影响 opts 的链路都能注入 HTML / `<img onerror>` |
| `#/menu`、`#/` | `CurrentWindow.getConfigUrl(key)` + `showMainView(key)` → 远端 URL；`openUrl(url,title,810,680)` | 服务端配置表（`/front/html/configs`）驱动；改表即让客户端加载任意页面 |
| `xotFullScreen` | `loadScript(resPath+"page1.js")`（@25167 取 resPath、@25326 拼路径）→ `<script>` 执行本地任意 JS；`eval('('+xhr.responseText+')')`（ajax 回调 @885072）；art-template 用 `new Function` 编译模板 | 离线资源目录 = 代码执行入口；`eval` 把 HTTP 响应体当 JS 求值（HTTPS 下需中间人才能利用，属设计缺陷） |
| `login.html` | `$dialogNotify({title:"<p>密码连续错误10次…</p><p>…</p>"})`、`$toaster({message})`；`createElement("script").src=`（顶象 CDN / CNZZ） | 标题走 HTML 注入（当前内容为本页常量，同一 API 若喂服务端串即为 XSS）；脚本注入仅浏览器路径 |
| `index.html` 壳 | 无 `v-html`、无 `eval` / `new Function`（1 MB 内 `eval(`=0、`new Function`=0）；`innerHTML` 16 处全在 Element Plus 内部 | Vite 侧攻击面小，主要靠 preload 桥 |

#### 6.2.1 串联结论（与 security.md 第 9 节口径一致）

- **(a)+(c) 是同一条链**：`#/separate?url=` 把远程页放进同进程 iframe，`webSecurity:false` 下该远程页可直接取 `parent.CurrentWindow.*` 以及 parent 上的 preload 对象；而 `#/dialog` 的 `innerHTML` 落在这个挂着 `../preload/index.js`、`contextIsolation:false` 的特权窗口里 → **服务端可控文案 → `<img onerror>` → 文件读写 / shell**，全程不需要新开窗、也不需要用户额外点击。
- **`page1.js` 不是单纯 XSS，而是持久化代码执行入口**：书籍包只校验 zip 密码、无签名（见 security.md §5），因此该脚本可被服务端下发、更新链或本地写权限替换，且每次进入离线讲解都会执行一次（`loadScript` 无缓存校验）。

### 6.3 反向通道（主进程 → 页面）

页面靠 `CurrentWindow.on(<event>)` / `Login.on(<event>)` 接收推送，事件全集（grep 统计）：

- `CurrentWindow`：`menu` `toolbar` `dialog` `resize` `fullscreen` `maximize` `unmaximize` `stayontop` `cloudStatus` `scoreUploaded` `showTool` `readAllMessage` `packageLoading`
- `Login`：`loging` `logined` `tokenLogined` `wechatLogined` `loginStateChanged`
- 页面 → 主进程：`CurrentWindow.emit("viewReady")`（3 个 pdf 页）、`emit("message", event)`（对话框 ok/cancel）、`emit("menu","",false)`

`window.Login.State` 语义（观测）：1=已登录、2=离线模式、其他=未登录。

---

## 7. 未验证 / 待办（交给其它角色）

1. ~~`.up366test.cn` 基址是否被改写~~ → 已由 recon/api.md 排除：`session.webRequest` 只拦 `redirect://getServiceTicket`，不重写 host。剩余问题：为何发布渠道选的是 test 域（构建配置层面）。
2. `getConfigUrl(key)` 的 key→URL 表从哪来、是否落盘、能否被远端改？→ 主进程 + 服务端。
3. `resPath` 指向的课程目录磁盘布局（`page1.js` 与 `questions/<qid>/` 是否真实存在）→ 需实际离线数据。
4. `/lib/xoteditorv2/` 静态资源映射由谁提供（站点根 or `local:` 协议）→ 主进程。
5. `6tmxa2VTDs.txt` 的用途。
6. `#/pdf/*` 是否真走 `printToPDF`（当前仅 `viewReady` 佐证）。

> 偏移口径与复核：本文全部 `@N` 已按 UTF-8 解码字符下标重新计算并逐条回读校验（0 处落空）；其中 6 处关键注入点（#/dialog innerHTML、login iframe 桥、明文记住密码、resPath→page1.js、eval 响应体）已由 security 独立复核，结论并入 security.md 第 9 节。渲染层存储中没有发现 TGT / u3t / UserSig 一类令牌（`saveData` 字面量 key 只有 `sessionData`、`userInfo`），凭证类敏感项只有 `userInfo` 的明文账号密码。
