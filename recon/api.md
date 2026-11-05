# up366 PC 学生端 — 网络层测绘与 API 全表（中文）

> 目标：`extract/app.u3a/dist/main/index.js` (3,901,675 B / 368 行，单行压缩)、`main/main-window.js` (4,336,747 B)、`preload/index.js` (4,136,635 B)、`utils/libup366.js`。
> 证据格式：`文件:字节偏移`（`off`），行号为压缩后行号（主逻辑集中在 line 228 / 255 / 257 / 70）。
> 三份 bundle 中**同一套代码被重复打包**（变量名不同：`dF/yR/yF`、`M2/…`、`vC/vf`、`rf/vf`），下文统一以 `main/index.js` 的偏移为准，必要时标注 `main-window.js`。

---

## 0. 一句话结论

- 传输层不是 axios，而是 **Electron `net.request` 的自研封装**（`vC` + `rf`），POST 默认 `application/x-www-form-urlencoded`。
- 每请求固定 7 个头：`user-agent: PC-Up366-Student 6.11.0`、`x-requested-with: PC`、`X-App-Name: student-pc`、`clientId: <MachineCode>`、`u3r: <每请求新随机 UUID>`、`u3t: <TGT>`、`Authorization: <恒为空串>`。
- **没有对每个请求做签名**；只有**登录/短信/上传任务成绩**这类敏感接口带一个 `ut` 字段：`ut = md5(ts).slice(0,10) + md5(payload + 关键词 + md5(ts)) + md5(ts).slice(10)`。
- 唯一真正的 HMAC 是文件元信息接口：`HMAC-SHA1(key="au66ssoI3nGr6DxCHk8KcpVsQ1wYNQId", msg="GETOBJECTschool"+fileIds) → base64`。
- 业务 host 的主要凭证是**登录 Cookie**：`UP366-C`、`CASTGC`（`Domain=.up366.cn`）。2026-11-05 实测仅 Cookie 即可访问 `courses` / `jobs` / `job_info` / `books` / `finished_tasks` / `wrongnote`，**仅 `u3t` 返回 `-21`**；`SESSION` 只作用于 `sso.up366.cn`。PC 客户端仍随请求发 `u3t`，并在 Cookie 缺失/过期、业务返回 `-21/-22` 时用它向 sso 换一次性 ST、以 `?ticket=` 重放（不是 JWT/Bearer）。下文各节写的“需要 u3t”应理解为“PC 客户端随请求携带 u3t”，不代表 u3t 单独可鉴权。
- 无 WebSocket / 无 SSE，全部是请求-响应 + 定时轮询。
- **字段级说明见 §1.11（认证 22 条）/ §1.12（教材任务 17 条）/ §1.13（文件及其它 15 条）/ §1.14（答题提交链路）。**

---

## 1. Endpoint 全表

**基址规则**：所有 URL 写死为 `https://xxx.up366.cn/...`，运行时由 Proxy 把字面量 `up366.cn` 替换为环境域名（`main/index.js:3556586` 起 `dF` 类的 `constructor(e){...new Proxy(this,{get:(r,i)=>{let n=r[i];return xe.isString(n)?n.replace("up366.cn",e):n}})}`）。

环境域名白名单（`main/index.js:626699` `function ts()`）：

| 环境 | 域名 | OAuth 路径前缀 |
|---|---|---|
| 生产 | `up366.cn` | `""` |
| UAT | `up366.com.cn` | `uat` |
| 演示 | `up366demo.cn` | `demo` |
| 测试 | `up366test.cn` | `test` |

来源优先级：`Xp().platform`（electron-store 配置）→ 正则 `/up366(demo|test|.com)*\.cn$/i` 校验 → 回退 `up366.cn`。

### 1.1 setup-api.up366.cn —— 配置 / 版本 / 健康 / 二维码 / 日志票据

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| setup-api.up366.cn | /front/health-status | GET | 启动前连通性探测，2 次尝试、每次 3.5s 超时，判 `res.code==0` | main-window.js:4333872 |
| setup-api.up366.cn | /front/pc/configs | GET | 远程配置/灰度（`sls`、`behavior-log-use-sls`、`jssdk2023`、`bookGray`、`bookOpenType`、`recorder`、`limitConfig`、`common`），返回 `{default,special[{areaIds,organIds,uids,schoolType,clientType,uidPercent,version,config}]}` 由客户端本地求交集 | 调用 main/index.js:3882686；求值 `aEe` main/index.js:3626245 |
| setup-api.up366.cn | /front/current-time | POST | 取服务器时间（OSS `RequestTimeTooSkewed` 时校正本机时钟，写入 `eV()/dM()`） | main/index.js:3571948 |
| setup-api.up366.cn | /front/aliyunoss/token/v2?organId=`{schoolId}` | GET | 取 OSS STS v2（bucket/endpoint/sts），结果缓存 60s | main/index.js:3577169 |
| setup-api.up366.cn | /front/appversion/latestVersion | GET | 查最新版本，参数 `{modelName,versionNo,username}`；`modelName=PC-UP366-TEACHER` 用于拉起教师端 | main/index.js:3572025 / 3896057 |
| setup-api.up366.cn | /front/app/version/list | GET | 版本列表，参数 `{modelJson,username}` | main/index.js:3572240 |
| setup-api.up366.cn | /front/qrcode/login-url | 推断 GET | 仅 `getQrCodeLoginUrl()` 把 URL 原样交给渲染层，主进程不自己发；由渲染层取二维码 | main/index.js:3895483 |
| setup-api.up366.cn | /front/qrcode/login/result | 推断 GET | 同上，二维码状态轮询地址 | main/index.js:3895536 |
| setup-api.up366.cn | /client/ali/log/config | POST | 日志上报票据：`{agentType:2}` → `{accessKeyId, secretAccessKey, securityToken, ...}`（阿里云 SLS） | main/index.js:597593 |
| setup-api.up366.cn | /client/userfeedback/spoken/save | POST | 语音反馈上传 | main-window.js:4261443 |
| setup-api.up366.cn | /front/organ/config | /front/aliyuntoken/getaliyuntoken | /client/organ/menu/list | /client/oss/upload/object/info | /client/up366/token | — | **配置项存在但主进程无调用点（死代码）** | 定义 main/index.js:3556586 起 |

### 1.2 sso.up366.cn —— 认证中心（TGT/ST 体系）

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| sso.up366.cn | /v2/tickets-v3 | POST | 用户名密码登录，body `{username, password(md5链), ut}`，成功返回 `code=-26, data.tgt` | main/index.js:3563553 |
| sso.up366.cn | /v2/code/tickets | POST | 短信验证码登录 `{mobile, verifyCode}` | main/index.js:3563032 |
| sso.up366.cn | /v2/auto/tickets | POST | **token 免密登录/自动登录**，body `{token, ut}` | main/index.js:3563988 |
| sso.up366.cn | /v2/weChat/state | POST | 取微信 OAuth 的 `appId/state` 与扫码 URL | main/index.js:3564278 |
| sso.up366.cn | /v2/weChat/info | POST | `{code,state}` → openId | main/index.js:3564489 |
| sso.up366.cn | /v2/weChat/tickets | POST | openId 换 TGT，body `{openId, ut}` | main/index.js:3564818 |
| sso.up366.cn | /v2/qq/state | POST | 取 QQ `appId/state` | main/index.js:3564385 |
| sso.up366.cn | /v2/qq/info | POST | `{code,state,redirectUrl}` → openId | main/index.js:3564598 |
| sso.up366.cn | /v2/qq/tickets | POST | openId 换 TGT，body `{openId, ut}` | main/index.js:3565213 |
| sso.up366.cn | /v1/serviceTicket | POST | **业务接口 401 后换一次性 ST**：body `{service:<目标 URL>}` → `{code:-24, data.serverTicket}`，随后以 `?ticket=<st>` 重放原请求 | main/index.js:3565443 |
| sso.up366.cn | /captcha.html | — | 未调用（死配置） | 定义 main/index.js:3556586 起 |

### 1.3 user-api.up366.cn —— 用户

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| user-api.up366.cn | /client/user/currentuser | POST | 登录成功后拉当前用户（`zm` 对象：uid/uuid/account/realName/type/schoolId/cityId/provinceId/countryId/schoolType…） | main/index.js:3566203 |
| user-api.up366.cn | /front/user/verify/send | GET(作为 query 传参) | 发送短信验证码，`{checkExists, mobile, ut}` | main/index.js:3562916 |
| user-api.up366.cn | /front/user/verify/check | /client/user/mobile/update | /client/user/pwd/check | /client/user/pwd/update | /client/user/search | — | 死配置（改密/绑手机/搜人在 PC 学生端未接线） | 定义 main/index.js:3556586 起 |

### 1.4 book-api / studytask-api / school / dsb —— 教材与任务

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| book-api.up366.cn | /client/books/buy-book | POST | 我的书架（PC 翻书 V11） | main/index.js:3566275 |
| book-api.up366.cn | /client/books/tree | POST | 教材目录树 `{bookId,version,bookType:1}` | main/index.js:3567012 |
| book-api.up366.cn | /client/v2/book/info | POST | 书籍详情 `{bookId,bookType:1,originalBookId}`，缓存 2h（`cacheTTL=72e5`） | main/index.js:3566813 |
| book-api.up366.cn | /client/custom/book/chapters | POST | 自定义章节目录 `{bookId,courseId}` | main/index.js:3567140 |
| book-api.up366.cn | /client/chapters/personalized/list | POST | 个性化章节 | main/index.js:3567246 |
| book-api.up366.cn | /client/chapters/list-question-file | POST | 章节题目文件 `{bookId,chapterId,fileId}` | main/index.js:3567526 |
| book-api.up366.cn | /client/task/linked | POST | 章节关联任务 `{bookId,taskId,type}`，缓存 2h | main/index.js:3567957 |
| book-api.up366.cn | /front/offline/download/books | POST | 离线可下载书籍列表 | main/index.js:3567449 |
| studytask-api.up366.cn | /client/book/plan | POST | 课程学习计划 | main/index.js:3567342 |
| studytask-api.up366.cn | /client/student/job/info | — | 死配置 | 定义 |
| school.up366.cn | /client/textbook/chapter/weight/list | — | 死配置 | 定义 |
| dsb-api.up366.cn | /client/book/finished-task/list | GET（PC）/ POST（H5） | 已完成任务列表 `{bookId}`（PC/H5 都不传 courseId） | main/index.js:3617392；H5 chunk-6b733b35.js@9507 |
| dsb-api.up366.cn | /client/student/task/answer/detail | GET | 任务答题详情（PC：`{courseId,bookId,taskId,uid}`；调用点 @3768493） | main/index.js:3561535、3572472、3768493 |
| dsb-api.up366.cn | /client/student/task/answer/detail/v2 | POST | 任务答题详情 v2（H5 路由参数 `{uid,bookId,taskId}`） | H5 chunk-6c496ba5.js@892819 |

### 1.5 study-api / xot-api / qlib-api —— 答题提交与试卷

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| study-api.up366.cn | /client/task/score/submit/v2 | POST | 学习任务成绩提交 `{tasksJson, submitType, ut}` | main/index.js:3579270 |
| study-api.up366.cn | /client/task/score/gzip/submit | POST + gzip | **离线队列批量补交**：body 为 gzip 后的 `{tasksJson, ut, submitType}`，头 `Content-Type: application/json;charset=UTF-8` + `Content-Encoding: gzip` | main/index.js:3827669 |
| xot-api.up366.cn | /client/onlinetesting/submit | POST | 在线测试（考试）成绩提交 | main/index.js:3579489 |
| xot-api.up366.cn | /client/onlinetesting/alldata | POST | 拉考试全量数据 | main/index.js:3752224 |
| xot-api.up366.cn | /client/paper/zipinfo | POST | 试卷包信息（取下载地址/校验） | main/index.js:3752598 |
| xot-api.up366.cn | /client/onlinemarking/aimarkdata/submit | /client/onlinemarking/testandpaperinfo | — | 死配置（智能批改为教师端功能） | 定义 |
| qlib-api.up366.cn | /client/paper/detail | GET | 试卷详情 `{paperId}` | main/index.js:3572432 |
| qlib-api.up366.cn | /client/questions/import/question-upload | /client/questions/import/editor-upload | — | 死配置 | 定义 |

### 1.6 mall-api / msg-api / pigai-api / homework-api / livefile-api

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| mall-api.up366.cn | /client/order/goods/validity | POST | 商品/订单有效性（买书前校验） | main/index.js:3566481 |
| mall-api.up366.cn | /front/goods/active-code | /client/goods/active-book | /client/cart/goods/add | — | 死配置（激活码/加购物车未在 PC 学生端接线） | 定义 |
| msg-api.up366.cn | /client/msg/list-of-pc | POST | 未读消息，`{status:0,"pager.currentPage":1,"pager.pageSize":999}` | main/index.js:3566345 |
| pigai-api.up366.cn | /client/result | GET | 批改结果 `{tid}`（导出实用文报告用） | main/index.js:3572355 |
| pigai-api.up366.cn | /client/batch/result | — | 死配置（批量版，被单条版取代） | 定义 |
| homework-api.up366.cn | /client/homework/test/publish | /test/course-info | /mark/student-list | — | 死配置（布置作业=教师端） | 定义 |
| livefile-api.up366.cn | /client/classfile/upload | POST multipart | 直播课件上传：`fileMD5` + `fileName` + `file` | main/index.js:3567734 |

### 1.7 fs.up366.cn —— 文件元信息与下载（唯一带 HMAC 的接口）

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| fs.up366.cn | /fileinfo/`{fileId}` | POST | 单文件信息 `{app:"school", auth: HMAC-SHA1}` | main/index.js:3575521 |
| fs.up366.cn | /fileinfo/list | POST | 批量（每次 ≤100 个 id）：`{fileids:"a,b,c", app:"school", auth}` | main/index.js:3576249 |
| fs.up366.cn | /download | GET | 实际下载地址由 `/fileinfo` 返回的 `data.downloadUrl` 决定（`http://` 会被强制升级为 `https://`）；支持 `Range` 断点续传、`md5` 校验、`miniCloudDownloadUrl` 中转 | main/index.js:3576697 `downloadByFileId` |

### 1.8 第三方 / 非 up366 域

| host | path | 方法 | 用途 | 证据 |
|---|---|---|---|---|
| fy.ismartin.com | /essay/assess/api | POST | 作文批改（第三方） | main-window.js:4280286 |
| fanyi.ismartlearning.cn | /correct | POST | 作文纠正（第三方） | main-window.js:4280684 |
| hw-rpt-cloud-api.`{platform}` | /client/teacher/test/score-sheet?testId=123 | POST | **隐藏探针**：每次请求该域的 URL 前，若 1 小时内未探过，先打一枪取 `code==1` 作能力探测 | main/index.js:3549068（`rf.start()` 内） |
| open.weixin.qq.com | /connect/qrconnect?appid=&redirect_uri=&response_type=code&scope=snsapi_login&state=#wechat_redirect | GET | 微信扫码登录页（`redirect_uri` = `www.up366.com/wechat-redirect.html?reUrl=3&platForm=<env>`） | main/index.js:3892541 |
| graph.qq.com | /oauth2.0/authorize?client_id=&redirect_uri=&response_type=code&state= | GET | QQ 登录页（`redirect_uri` = `www.up366.com/qq-redirect.html?reUrl=3&platForm=<env>`） | main/index.js:3892456 |
| `{bucket}.{defaulthostid}` / *.aliyuncs.com | OSS | PUT/POST | 课件/音频/图片直传（ali-oss SDK，`sts.aliyuncs.com` 取凭据） | main/index.js:3556066 |
| cdn-aiyun-up366-bucket1.up366.cn | / | GET | OSS 自定义 CDN 域（死配置） | 定义 |
| cdn-ws.up366.cn | /pcclient/font | GET | 字体下载（死配置） | 定义 |

### 1.9 私有云（minicloud，管理员配置的局域网服务）

`_L` 类（`er`），地址来自 `config.ini` / `cloud.config`，形如 `http://{miniCloudIP}:{miniCloudPort}`（`main/index.js:3534188`）：

| path | 方法 | 用途 |
|---|---|---|
| /minicloud/status?type=simple | GET | 每 10s 轮询，`code==0` 时按 `status` 映射 2/3/4（离线/在线…） |
| /minicloud/upload | POST | 私有云上传（`{bucketName,endpoint,objectName,fileData}` multipart） |
| /minicloud/download | GET | 私有云下载中转（`?url=<encoded>` 或 `?back=1/local=1`） |
| /minicloud/update/list | GET | 私有云版本列表 |
| /minicloud/update?model=STUDENT-PC-MINICLOUD-UPDATE | GET | 私有云更新包信息 |

### 1.10 纯网页地址（非 API，供 WebView / `shell.openExternal`）

`student.up366.cn/#/`、`/#/user`、`/#/textbook`、`/#/mybook`、`/#/message`、`/#/wrongNote`、`/dictHome.html`、`/feedback.html`、`/selfstudy.html`、`/similar.html`、`/textbook.html`；`me.up366.cn/center/market.html`、`/center/student/course/answerAdd.html`、`/front/cart.html`、`/xot/textbook.html`；`mall.up366.cn/ismart/center/myCoupons.html`；`smartpaper.up366.cn`。

### 1.11 字段级详情：认证 / 用户 / 配置（22 条）

> 每条含：用途、鉴权、调用点、请求参数表、响应、备注、字节偏移证据。偏移口径为 UTF-8 解码后字符下标。

#### GET /front/health-status — 服务健康检查
- **域**：setup-api.up366.cn
- **鉴权**：匿名（登录前即调用；请求头 u3t 为空串）
- **用途**：判断服务端是否可达，用于在线/离线模式切换前的探测。
- **调用点**：MainWindow.checkOnline(cb) @main-window.js:4333680
- **请求参数**：无
- **响应**：读**顶层** `code`（不是 result.code），`code===0` 视为在线；其余或超时视为离线。重试 2 次，单次超时 3500ms。
- **备注**：走原始 `http.get`，不经过 `service.post/get`，因此不参与 -21/-22 的 ST 兜底重试。bundle 内只看到定义，未找到调用方（应为主进程入口或渲染层 IPC 触发）。
- **证据**：main-window.js@4333680、main/index.js@3556599

#### GET /front/pc/configs — PC 端灰度配置
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t（登录成功后调用）
- **用途**：登录完成后拉取客户端功能开关（日志采样、JSSDK、教材灰度、录音、限制策略等）。
- **调用点**：Qy() 登录收尾函数 @3882686（`Ge.getByName("appConfigs")`）
- **请求参数**：无
- **响应**：`result.code!==0` 直接抛 `获取配置数据失败code(…)`。`data` 只取下列 8 个 key：`sls`、`behavior-log-use-sls`、`jssdk2023`、`bookGray`、`bookOpenType`、`recorder`、`limitConfig`、`common`。每个 key 的结构为 `{default:{…}, special:[…]}`，special 项命中的筛选字段为 `areaIds`(匹配 cityId/countryId/provinceId)、`organIds`(schoolId)、`uids`(uid)、`schoolType`、`clientType`(平台与 platform-arch)、`uidPercent`(uid%100 小于阈值才命中)、`version`、`config`(命中后 Object.assign 覆盖 default)。最终业务读取 `o.sls.enable`、`o.sls.level`、`o.behaviorlogusesls.enable`。
- **备注**：结果整形后写入内存 `AppInfo.setMemoryData("appConfigs")`，由 KI() 读取复用；key 名里的短横线会在整形时被删除（`behavior-log-use-sls` → `behaviorlogusesls`）。
- **证据**：main/index.js@3882686、main/index.js@3626245（整形函数 aEe）

#### POST /front/current-time — 取服务器当前时间
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t（无显式鉴权判断）
- **用途**：本机时钟偏差导致 OSS 上传被拒时，用服务端时间校准本地时间基准。
- **调用点**：mF.getServerTime() @3571912；唯一调用点在上传重试逻辑 @3556066
- **请求参数**：无
- **响应**：`result.code!==0` 或 `data` 为空则抛出原始 OSS 异常（不继续重试）；否则取 `data.currentTime` 交给 `eV()→dM()` 设置时间基准，10 位当秒、13 位当毫秒。
- **备注**：仅在阿里云返回 `RequestTimeTooSkewed` 时触发，之后重试一次上传。走 `postByName`，即 POST 且无 body。
- **证据**：main/index.js@3571912、main/index.js@3556066

#### GET /front/aliyunoss/token/v2?organId= — 取 OSS STS 凭证
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t（organId 取自当前用户）
- **用途**：上传/下载文件前获取阿里云 OSS 的临时凭证与 bucket 域名。
- **调用点**：Ge.getOssTicket() @3577124；消费方 `_o.getOSSInfo()` @3554086
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | organId | string/number | 学校 ID，直接取 `global.CurrentUser?.schoolId`；未登录时拼成空串。走 query，不是 body |
- **响应**：`result.code!==0` 直接 return（不抛错，调用方拿 undefined）。`data` 交给 `new wi("biz", data)`，按 bucketId 取出 `ossBucketInfo`，读取 `ossBucketInfo.currentTime`（判活）、`ossBucketInfo.bucketname`、`ossBucketInfo.defaulthostid`（拼 `https://{bucketname}.{defaulthostid}/{objectName}`）。
- **备注**：缓存 60s（`now - ossBucketInfo.currentTime > 6e4` 才重取）；走 GET + query string。
- **证据**：main/index.js@3577124、main/index.js@3554086、main/index.js@3559007

#### GET /front/appversion/latestVersion — 查询最新版本
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t（未登录时 username 传空）
- **用途**：客户端/插件/安装包的自升级检查。
- **调用点**：mF.getUpdateConfig() @3571976；被 UpdateBase::checkUpdate() @3678711、InstallPackageDownload::getConfigInfo() @3717819、openUp366Teacher() @3896057 调用
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | modelName | string | 型号标识。主程序由 modelName() 生成，如 `PC-UP366-TEACHER`；win32 下另有 `PC-UP366-SMARTPAPER-5.0`/`…-UPDATE-5.0` |
  | versionNo | string | 本地版本号，`String(localVersionNo())`；openUp366Teacher 分支不传 |
  | username | string | 当前账号，`CurrentUser?.account`；未知时为空串 |
- **响应**：`result.code` 为 0 或 100 才认为请求有效。`data.status`（1=有更新，其他=无）、`data.forceUpgrade`（3=强制升级，>1=提示升级）、`data.version`（数字版本号，与本地相等时把 status 置 0）、`data.versionName`、`data.url`（安装包地址）、`data.apkSize`、`data.updateLog`。
- **备注**：错误码表把 100 定义为「找不到型号」，升级流程把 100 当作「没有可用的更新包」。走 GET，参数在 query 里。
- **证据**：main/index.js@3571976、main/index.js@3678711、main/index.js@3896057、main/index.js@3559283

#### GET /front/app/version/list — 批量查询最新版本
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t
- **用途**：一次性检查多个子型号（主程序 + 各插件）的最新版本，减少请求数。
- **调用点**：mF.getUpdateConfigList() @3572091；调用方 U3AppUpdate::checkUpdateList() @main-window.js:4164678
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | modelJson | string | `JSON.stringify([{modelName, versionNo}, …])`，元素仅这两个字段，来自各更新器的 modelName()/localVersionNo() |
  | username | string | 当前账号 |
- **响应**：`result.code===0` 且 `data` 为数组时，按 `o.modelName` 建 map，逐项取 `o.lastVersion` 交给 `checkUpdateFromInfo()`（其字段结构与 latestVersion 的 data 相同）。
- **备注**：走 GET，JSON 字符串塞 query。私云模式（`cloudEnabled`）改打 `miniCloudUpdateList`，不是本域接口。bundle 内无其它调用方。
- **证据**：main/index.js@3572091、main-window.js@4164678、main/index.js@3559359

#### ? /front/qrcode/login-url — 扫码登录二维码地址
- **域**：setup-api.up366.cn
- **鉴权**：匿名（登录前使用）
- **用途**：取二维码里要编码的登录地址（扫码后由手机端确认，PC 轮询 result 接口）。
- **调用点**：`getQrCodeLoginUrl()` @3895490 / main-window.js:4153388，仅把 URL 常量下发给本地登录页（`index.html`）渲染层
- **请求参数**：代码中未构造，主进程只下发 URL 字符串，实际请求由渲染层发起
- **响应**：代码中未读取，主进程不解析
- **备注**：请求方法在 bundle 中无法确认（未见到调用代码），参数与返回结构需在渲染层/登录页确认。登录页为本地资源，未包含在本次提取范围内。
- **证据**：main/index.js@3561118、main/index.js@3895490、main-window.js@4153388

#### ? /front/qrcode/login/result — 扫码登录轮询
- **域**：setup-api.up366.cn
- **鉴权**：匿名（登录前使用）
- **用途**：PC 轮询扫码状态，拿到登录结果后再走 token 登录。
- **调用点**：`getQrCodeLoginStatusUrl()` @3895543 / main-window.js:4153388 同一处，下发给渲染层
- **请求参数**：代码中未构造，透传给渲染层
- **响应**：代码中未读取，主进程不解析
- **备注**：同上一节，方法无法从 bundle 确认。另注：token 登录成功后客户端上报的登录方式记为 `LoginByQRCode`（见 /v2/auto/tickets），说明扫码登录最终复用 token 登录接口。
- **证据**：main/index.js@3561180、main/index.js@3895543

#### POST /client/ali/log/config — 取日志服务(SLS)票据
- **域**：setup-api.up366.cn
- **鉴权**：走全局 `AppInfo.httpRequest`，鉴权方式在 bundle 中不可见
- **用途**：初始化阿里云 SLS 客户端，上传客户端行为日志。
- **调用点**：Vp.getTicket() @597546（静态方法）
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | agentType | number | 固定常量 2，代码中写死 |
- **响应**：`result.code!==0` 抛 `获取Ticket失败: ${result.msg}`；`data` 直接返回，字段 `accessKeyId`、`accessKeySecret`、`securityToken`、`endpoint`，用于 `new SLS({…, endpoint:"https://"+endpoint, apiVersion:"2015-06-01"})`。
- **备注**：走 `AppInfo.httpRequest("post", url, {agentType:2})`，该函数不在本次提取的 bundle 内，因此无法确认是否复用统一请求头（u3t 等）。票据过期策略：日志上传报错 message 含 `expired` 或 `invalid` 时，清空 `this.sls`/`this.ticket` 下次重取。
- **证据**：main/index.js@597546、main/index.js@597592

#### POST /client/userfeedback/spoken/save — 上传口语录音反馈
- **域**：setup-api.up366.cn
- **鉴权**：随请求携带 u3t（登录后使用）
- **用途**：用户对口语评测结果不满意时，打包录音与评测数据上报，供后台人工复核。
- **调用点**：口语反馈上传流程 @main-window.js:4261455（`Ie.postByName("saveSpokenFeedback", …)`）
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | courseId | 透传 | 取 `this.data.courseId` |
  | contactInfo | string | 代码中固定空串 |
  | osVersion | string | 代码中固定空串 |
  | mobileModel | string | 代码中固定空串 |
  | taskInfo | 透传 | 取 `this.data.taskInfo` |
  | score | 透传 | 取 `this.data.score` |
  | evaluation | 透传 | 评测结果对象，由调用方传入 |
  | zippath | string | zip 包在 OSS 上的完整地址，由 `getOssHost(DefaultBucketId, "feedback/spoken-*.zip")` 拼出 |
  | engineVersion | string | `PluginManager.getPlugin("ases")?.version` |
- **响应**：`result.code===0` 视为上传成功并弹「上传成功」；否则弹「上传失败」并记错误日志。
- **备注**：必须先 `smartUploadFile` 上传成功（`result.code===0`）才会调本接口；三个 `*` 字段是留空占位，不再采集。
- **证据**：main-window.js@4261455、main/index.js@3562367

#### POST /v2/tickets-v3 — 账号密码登录（签发 TGT）
- **域**：sso.up366.cn
- **鉴权**：匿名（此时 u3t 为空）
- **用途**：用户名+密码换取 TGT，后续所有接口用 TGT 填 u3t 头。
- **调用点**：CF.login(username, password) @3563185；IPC 入口 @3885790
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | username | string | 明文账号，取自 IPC 参数 `e.userName` |
  | password | string | **不是明文**：`md5(md5(明文密码) + "fa&s*l%$k!fq$k!ld@fjlk")`，32 位小写 hex。盐值硬编码在客户端 |
  | ut | string | `md5(ts).slice(0,10) + md5(username + password + "loginEncryptionParameter" + ts) + md5(ts).slice(10)`，共 64 字符；ts=`Date.now().toString()`，md5 为 32 位小写 hex |
- **响应**：`result.code===-26`（生成TGT成功）判为成功，取 `data.tgt` 写入请求头 u3t，并把明文 username/明文 password 暂存 `service.loginUsername/loginPassword` 供 ST 过期后自动重登；同时校验 `data.uType` 必须为 102（学生），否则客户端伪造错误码 1001「下载教师客户端」。
- **备注**：失败时只记日志并原样返回 result。登录前会清掉本地 `Config/Token`；`e.remember` 为真时把明文 `{username,password}` 存进 `Config/UserInfo`（本地明文落盘）。成功码 -26 的语义来自错误码表 @374154（-26 生成TGT成功 / -27 生成TGT失败 / -29 账号锁定 / -30 账号禁用 / -31 登录设备数过多）。
- **证据**：main/index.js@3563185、main/index.js@3885790、main/index.js@3556776、main/index.js@374154

#### POST /v2/code/tickets — 短信验证码登录
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：手机号+短信验证码登录。成功后拿到的 token 还要再走一次 token 登录。
- **调用点**：CF.verifyCodeLogin(body) @3562970；IPC 入口 @3887102
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | mobile | string | 手机号，IPC 参数 `e.mobile` |
  | verifyCode | string | 短信验证码，IPC 参数 `e.verifyCode` |
  | ut | — | **无**。这是唯一不构造 ut 的登录接口，body 由渲染层原样透传 |
- **响应**：`result.code===-26` 成功，读 `data.uType`（须为 102）与 `data.token`；随后内部再调 `loginByToken({token})` 完成真正登录。其它码原样回传渲染层。
- **备注**：直接 `http.post`，不走 `service.post`，因此**不会**触发 -21/-22 的 ST 兜底。异常统一映射为客户端错误码 -501（并把原始 `code/message/name` 塞进 `result.err`）。
- **证据**：main/index.js@3562970、main/index.js@3887102、main/index.js@3561693

#### POST /v2/auto/tickets — Token 自动登录
- **域**：sso.up366.cn
- **鉴权**：匿名（凭 token 自身鉴权）
- **用途**：记住登录态后的自动登录；也是扫码登录、短信登录的最终落地接口。
- **调用点**：CF.loginByToken(token) @3563823；IPC 入口 @3887804
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | token | string | 本地持久化的登录 token（`Config/Token`） |
  | ut | string | `md5(ts).slice(0,10) + md5(token + "loginEncryptionParameter" + ts) + md5(ts).slice(10)` |
- **响应**：`result.code===-26` 成功，取 `data.tgt` 写入 u3t，并缓存 `service.token=token`；`result.code===-39`（无效的Token）时删除本地 `Config/Token`；`data.uType` 必须 102，否则返回客户端错误 -999。
- **备注**：登录方式上报固定为 `LoginByQRCode`；`isAutoLogin` 或 `saveTime` 为真时把 token 持久化，否则只在内存里用。错误码 -39 的语义务必与 -26 区分：-39 表示已失效、应清本地 token。
- **证据**：main/index.js@3563823、main/index.js@3887804、main/index.js@3561248

#### POST /v2/weChat/state — 取微信扫码登录参数
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：拿公众号/开放平台的 appId 与 state，拼 `open.weixin.qq.com/connect/qrconnect` 授权页。
- **调用点**：CF.getWechatState() @3564212；调用点 weChatPreLogin() @3888917
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | wechat | string | 固定字面量 `"wechat"`，仅用于标记登录渠道 |
- **响应**：`result.code!=0` 直接弹「网络错误」；`data.appId`、`data.state` 拼进授权 URL 的 `appid`、`state`，`redirect_uri` 用 `config.weChatRedirect`（按平台加上 test 后缀）并附带 `reUrl=3&platForm={test|uat|demo|}`。
- **备注**：调用前先清除本地 Token；有 2s 防抖（`lastOpenUrlTime`）。平台到环境标识的映射：up366.cn→""、up366.com.cn→uat、up366demo.cn→demo、up366test.cn→test。
- **证据**：main/index.js@3564212、main/index.js@3888917、main/index.js@3556876

#### POST /v2/weChat/info — 微信 code 换 openId
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：微信授权回调带回 code/state，换取用户 openId。
- **调用点**：CF.getWechatInfo(code, state) @3564421；IPC 入口 weChatLogin(code, state) @3889909
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | code | string | 微信授权码，来自回调页 IPC 参数 |
  | state | string | 与 /v2/weChat/state 下发的一致 |
- **响应**：`result.code===1` 时不直接登录，把 `data.openId` 回传给页面由页面继续（对应未确认/未绑定等场景）；`result.code!==0` 弹「网络错误」；`result.code===0` 时取 `data.openId` 进入 openId 登录。
- **备注**：code 与 state 都由渲染层从回调 URL 里解析后经 IPC 传入，主进程不做校验。
- **证据**：main/index.js@3564421、main/index.js@3889909、main/index.js@3556927

#### POST /v2/weChat/tickets — 微信 openId 登录
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：用 openId 换 TGT，完成微信扫码登录。
- **调用点**：CF.wechatLogin(openId) @3564654；调用点 @3890288、@3891153（weChatLoginByOpenId）
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | openId | string | 上一步 /v2/weChat/info 返回的 `data.openId` |
  | ut | string | `md5(ts).slice(0,10) + md5(openId + "loginEncryptionParameter" + ts) + md5(ts).slice(10)`，与 token 登录同构，仅前缀换成 openId |
- **响应**：`result.code===-26` 成功，存 `data.tgt` 与 `service.openId`；`data.uType` 必须 102，为 103 时弹「当前微信已绑定家长账号，无法登录学生端」，其余弹「教师账号」。`result.code===1` 在某些入口被解释为「您还未绑定微信」。
- **备注**：openId 会被缓存进 `service.openId`，用于 ST 过期后的自动重登（优先级低于 token）。
- **证据**：main/index.js@3564654、main/index.js@3890288、main/index.js@3556976

#### POST /v2/qq/state — 取 QQ 扫码登录参数
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：拿 QQ 互联的 appId 与 state，拼 `graph.qq.com/oauth2.0/authorize` 授权页。
- **调用点**：CF.getQQState() @3564323；调用点 @3892173
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | qq | string | 固定字面量 `"qq"`，标记登录渠道 |
- **响应**：`result.code!=0` 弹「网络错误」；`data.appId`、`data.state` 拼进授权 URL。`redirect_uri` 用 `config.qqRedirectUrl`（**注意是 http 开头**，本地登录页会自行 replace 成 https），附带 `reUrl=3&platForm={…}`。
- **备注**：与微信版并列实现，环境映射规则一致。
- **证据**：main/index.js@3564323、main/index.js@3892173、main/index.js@3561748

#### POST /v2/qq/info — QQ code 换 openId
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：QQ 授权回调后换取 openId。
- **调用点**：CF.getQQInfo(code, state, redirectUrl) @3564532；IPC 入口 qqLoginInfo(code, state, redirectUrl) @3892984
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | code | string | QQ 授权码，来自回调页 |
  | state | string | 与 /v2/qq/state 下发的一致 |
  | redirectUrl | string | 回调页地址，由渲染层传入（微信版无此字段） |
- **响应**：`result.code===1` 弹「您还未绑定QQ号，该QQ号无法登录！」并中止；`result.code!==0` 弹「网络错误」；`result.code===0` 取 `data.openId` 进入 openId 登录。
- **备注**：比微信版多一个 redirectUrl 参数，这是两个渠道唯一的入参差异。
- **证据**：main/index.js@3564532、main/index.js@3892984、main/index.js@3561848

#### POST /v2/qq/tickets — QQ openId 登录
- **域**：sso.up366.cn
- **鉴权**：匿名
- **用途**：用 openId 换 TGT，完成 QQ 登录。
- **调用点**：CF.qqLogin(openId) @3565053；调用点 @3893399
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | openId | string | 上一步 /v2/qq/info 返回的 `data.openId` |
  | ut | string | `md5(ts).slice(0,10) + md5(openId + "loginEncryptionParameter" + ts) + md5(ts).slice(10)` |
- **响应**：`result.code===-26` 成功，存 `data.tgt` 与 `service.openId`；否则弹「网络错误」。代码中未对 QQ 渠道单独校验 `data.uType`。
- **备注**：与微信同构，仅 URL 不同。成功后上报登录方式 `LoginByQQ`。
- **证据**：main/index.js@3565053、main/index.js@3893399、main/index.js@3561892

#### POST /v1/serviceTicket — 用 TGT 换 ST
- **域**：sso.up366.cn
- **鉴权**：需要有效 u3t（TGT）；无 TGT 返回 -23
- **用途**：业务接口返回「没有登录(-21)」或「ST 校验失败(-22)」时，用 TGT 换一张某个具体业务接口的 service ticket，再带 ticket 重放原请求。
- **调用点**：CF.getServiceTicket(service, autoReLogin=false) @3565443；由统一传输层 `post()` @3574197 与 `get()` @3574750 在 `result.code∈{-21,-22}` 时调用
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | service | string | **触发失败的那个接口的完整 URL**，含原有 query string（例：`https://book-api.up366.cn/client/books/tree?bookId=1`）。取值规则就是失败请求本身的 URL，不做裁剪、不做域名改写 |
- **响应**：`result.code===-24`（生成ST成功）取出 `data.serverTicket`，拼成 `{原URL}?ticket={serverTicket}`（原 URL 已含 `?` 则用 `&`）重放一次；其它码不重试。`autoReLogin=true` 且首次没拿到 -24 时，会按 **token → openId → 账号密码** 的顺序重登一次（三者都为空则不重登），然后无条件重取一次 ST；第二次仍不是 -24 就记 `为用户{loginUsername}获取st失败` 并返回这一次的**登录结果**（不是 ST 结果）。
- **备注**：本接口**不走** `service.post`，用裸 `http.post`，避免自身再触发兜底递归；不带 ut。传输层对所有业务请求的统一失败码映射：-21 没有登录、-22 ST校验失败、-28 请求异常（网络层 catch 后包装）、-999 客户端错误。
- **证据**：main/index.js@3565443、main/index.js@3574197、main/index.js@3574750、main/index.js@3556819

#### POST /client/user/currentuser — 取当前登录用户
- **域**：user-api.up366.cn
- **鉴权**：需要 u3t（登录成功后立即调用）
- **用途**：拉取当前账号的完整用户档案，构造全局 CurrentUser 并缓存学校/地区等维度。
- **调用点**：hF.getCurrentUserInfo() @3566156；唯一调用点 Qy() @3882686
- **请求参数**：无（不带任何 body，身份完全靠 u3t 头）
- **响应**：`result.code!==0` 抛 `获取用户数据失败code(…)`。`data` 读取字段：`uuid`、`uid`、`realname`、`username`、`organId`（学校 ID）、`organName`、`schoolType`、`utype`（账号类型）、`mobile`、`reserved1`、`headPhoto`（头像）、`countryId`、`provinceId`、`cityId`。
- **备注**：登录流程强依赖，失败即中断登录；成功后把 `{username, uuid}` 存进 `CacheData/last_userinfo`，并把原始响应整体塞进 `CurrentUser.data`。字段名是**小写 realname/organId/headPhoto**，与其它接口的命名风格不一致，勿混用。
- **证据**：main/index.js@3566156、main/index.js@3882686、main/index.js@3584280（CurrentUser 构造）、main/index.js@3557202

#### GET /front/user/verify/send — 发送短信验证码
- **域**：user-api.up366.cn
- **鉴权**：匿名
- **用途**：短信验证码登录时发送 6 位验证码，同时可探活手机号是否已注册。
- **调用点**：CF.sendVerifyCode(mobile, checkExists) @3562783；IPC 入口 @3886850（写死 checkExists="2"）
- **请求参数**：
  | 字段 | 类型 | 说明 |
  |---|---|---|
  | mobile | string | 手机号 |
  | checkExists | string | 固定 `"2"`，含义为校验号码是否已存在（具体取值语义代码中只有字面量） |
  | ut | string | `md5(ts).slice(0,10) + md5(mobile + "sendSMSToken" + ts) + md5(ts).slice(10)`，盐串是 `sendSMSToken`，与登录接口的 `loginEncryptionParameter` 不同 |
- **响应**：读 `result.code` 与 `result.msg`，两者被直接注入渲染层回调 `window.onSendVerifyCode(code, "msg")`（注意 msg 未做转义，直接字符串插值）。
- **备注**：走 `getByName`，即 GET，参数在 query string 里。另有配套 `/front/user/verify/check` 校验接口与一组 `getUserAPIAccountUT()` 生成的 ut（`md5(account + "_!@#usertoken!@#_" + ts)` 包裹），用于 account 维度的匿名接口，与本接口的 ut 盐值不同。
- **证据**：main/index.js@3562783、main/index.js@3886850、main/index.js@3559428、main/index.js@3563065

---

### 1.12 字段级详情：教材 / 任务 / 评测（17 条）

#### POST /client/books/buy-book — 我的书架（已购教材列表）

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t（未登录时 `u3t` 为空串；服务端返回 -21/-22 会触发自动取票重试）
- **用途**：登录成功后拉取当前学生名下全部可用教材/绘本，作为本地书架单例（`Books`）的初始化数据源。全量取一次，之后切书走本地缓存。
- **调用点**：`hF.getMyBooks()` @3566236；由 `Books.init()` @3643187 调用
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| （无） | — | `postByName("getMyBookOfPCFlipbookV11")` 未传第二参数，实际发出 POST 空 body，不带 Content-Type，不进 form 序列化分支 |

- **响应**：`result.code`（0 = 成功）；`data` 为数组，逐项交给 `newFlipBook()`。主进程只显式读 `bookId`（去重键，`this.books.get(t.bookId)` 已存在则跳过）与 `packType`（`packType==9` 走 2025 版阅读器，@3645647），其余字段原样作为 `bookInfo` 透传。
- **备注**：无缓存、无 ut、无分页。除 `bookId`/`packType` 外没有任何字段被校验，结构以服务端为准。
- **证据**：main/index.js@3566236（调用）、3558011（URL 常量）、3643187（响应遍历）、3645647（packType 分支）

#### POST /client/books/tree — 教材目录树（章 / 页 / 任务）

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：按教材版本拉取整棵目录树（章 → 页 → 任务），是阅读器渲染与任务调度的主数据。带上本地缓存里的 `version` 做增量比对，版本一致就复用本地目录。
- **调用点**：`EF.getFlipbookChapters(bookId, version)` @3566961；调用方 `loadData()` @3621108（传缓存版本）、`loadChapter()` @3622285（传 `""` 强制取最新）、IPC 代理 `getFlipbookOfChapters` @3805998
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID |
| version | string | 客户端已缓存的目录版本号；传 `""` 表示不做版本比对、直接取最新 |
| bookType | number | 固定常量 `1`，硬编码在方法内，不对外开放 |

- **响应**：
  - `data.version`——目录版本号；与本地缓存不同才覆盖（`this.bookCatalog?.version!=r.data?.version` @3621306）
  - `data.book.configs.customFlag`——`==1` 时追加请求 `/client/custom/book/chapters`（@3621436）
  - `data.chapters[]` / `data.pages[]` / `data.tasks[]`——每项至少含 `id`、`parentId`，由 `initChapter()` @3617132 建树
  - 整份 `data` 直接落盘为本地目录缓存

- **备注**：无 ut。`version` 是本接口唯一的增量机制，注意 `""` 与具体版本号语义不同。
- **证据**：main/index.js@3566961、3558086、3621108、3621306、3621436、3617132

#### POST /client/v2/book/info — 教材详情

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：按 `bookId` 取单本教材的完整元信息（封面、有效期、资源包 ID、端能力开关），是打开阅读器前的必经请求。
- **调用点**：`EF.getBookOfPc(bookId, originalBookId="")` @3566641；调用方 `Books.addBook()` @3644378、JSSDK 打开链路 @3740607 / @3850272
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID |
| bookType | number | 固定常量 `1`，硬编码 |
| originalBookId | string | 原书 ID，换版本/溯源用；普通场景传 `""` |

- **响应**（客户端显式读取的字段，映射表 @3644509）：

| 字段 | 说明 |
| --- | --- |
| bookId / bookName | 教材标识与名称 |
| goodsId | 商城商品 ID，客户端做 `.toString()`（@3644561） |
| goodsImg / goodsVerticalImg | 横版、竖版封面 |
| startDate / endDate / validDays | 有效期区间与有效天数；本地 `addTime` 实际取的是 `startDate` |
| stageId / subjectId / categoryType | 学段、学科、类目 |
| packType | 资源包形态；`9` = 2025 版、`7` = 2019 版、`<7` = 2016 版（映射 @3639300） |
| fileId / pcFileId / pcVersion / pcRevision / mobileRevison | 资源包标识与 PC / 移动端版本号 |
| spId / bookType / toPC / toMobile / isShelf / isGoods | 售卖与端能力开关 |
| goodsPrice / password / autherName / openUrl | 价格、加密口令、作者、外链 |
| resourceList | 资源清单（含 `dirChapters`），后续取章节题目时用 |

- **备注**：**有缓存**——key=`getBookInfoOfPC_${uid}_${bookId}_${originalBookId}`，TTL `72e5` ms（2 小时），仅 `result.code==0` 时写缓存（@3566889）；`EF.clearCache()` 可整体清空。无 ut。
- **证据**：main/index.js@3566641、3558157、3644509、3566889

#### POST /client/custom/book/chapters — 自定义章节

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：教材带"自定义目录"时（`book.configs.customFlag==1`），按课程补充教师/机构配置的章节，与标准目录合并展示。
- **调用点**：`EF.getFlipbookCustomChapters(bookId, courseId)` @3567083；调用方 @3621475（`loadData`）、@3622547（`loadChapter`）
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID |
| courseId | number | 当前课程 ID，取自 `parent.currentCourse.courseId` |

- **响应**：`data` 整体赋给 `this.customCatalog`；主进程只读 `data.chapters[].chapterId`，据此把自定义章节挂到标准章节上（`this.customCatalog?.chapters?.forEach` @3618407）。
- **备注**：**条件请求**——必须先拿到 `/client/books/tree` 响应里 `book.configs.customFlag==1` 才会发。响应随目录整份落缓存。无 ut。
- **证据**：main/index.js@3567083、3558221、3621436、3621475、3618407

#### POST /client/chapters/personalized/list — 个性化章节列表

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：拉取服务端为学生个人定制的章节集合（个性化推荐 / 专属练习），结果整体交给渲染层。
- **调用点**：`EF.getFlipbookPersonalized(bookId)` @3567193；调用方 `updatePersonalized()` @3622095
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID |

- **响应**：`data` 整体赋给 `this.personalized`，对外只暴露 getter `Personalized` @3615321；主进程**不解读任何字段**，原样落本地缓存后交给渲染层。
- **备注**：无版本参数，每次调用都是全量。失败时 `updatePersonalized` 返回 `!1` 并沿用旧缓存。无 ut。
- **2026-11-05 实测**：测试账号书架 4 本书全部返回 `code=-4,msg=no_auth_zuul`；`updatePersonalized()` 对非 0 直接返回 false，因此这是账号/书籍权限而非协议错误。路径与 `{bookId}` 参数正确。
- **证据**：main/index.js@3567193、3558294、3622095、3615321

#### POST /client/book/plan — 课程学习计划（章节开关）

- **域**：studytask-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：取某课程在某本书上的学习计划，客户端只用它判断"哪些章节被关闭"，关闭的章节在建树时直接跳过。
- **调用点**：`EF.getPlanOfCourse(courseId, bookId, bookType=1)` @3567291；调用方 `loadCoursePlan()` @3620269
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID（对应函数**第二个**入参） |
| courseId | number | 课程 ID（对应函数**第一个**入参）；`>0` 才会发请求 |
| bookType | number | 默认 `1`，调用点未覆盖 |

- **响应**：`data` 是数组，用 `new wi("chapterId", e.data)` 建成以 `chapterId` 为键的字典（@3620492）；主进程只读 `isClose`，为真则整章跳过（@3617661 / @3622963）。
- **备注**：`currentCourse.courseId` 缺失或 `<=0` 时**不发请求**，计划表置空数组。无 ut。
- **证据**：main/index.js@3567291、3558376、3620269、3620492

#### POST /front/offline/download/books — 可离线下载教材白名单

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：获取允许离线缓存的教材 ID 白名单，用于阅读器"下载到本地"的可用性判断。
- **调用点**：`EF.getOfflineBooks()` @3567405；调用方 `updateCanOfflineBooks()` @3645210
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| （无） | — | 未传任何参数，POST 空 body |

- **响应**：`data.bookIds`——字符串数组。`getCanOfflineBooks()` @3645377 只读这一个字段，并用 `xe.isArray` 校验，非法则视为空。
- **备注**：**有本地持久化缓存**——`result.code==0` 时整份 `data` 写入 SQLite `Config` 表，name=`OfficeBooks`（@3645219）。读取优先用该缓存，网络失败不阻塞。
- **证据**：main/index.js@3567405、3558603、3645210、3645377

#### POST /client/chapters/list-question-file — 章节题目文件

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：取某章节对应题目文件中的题目列表，用于随堂练习类章节动态加载题目。
- **调用点**：`EF.getChapterQuestions(bookId, chapterId, fileId)` @3567479；调用方 @3638786
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID（调用方传 `this.flipbook.Id`） |
| chapterId | string | 章节 ID（调用方传 `this.Id`） |
| fileId | string | 题目文件 ID，取自本地目录缓存的 `chapterInfo.resourceList.dirChapters[0]`；取不到时调用方直接返回错误"资源包下载地址不存在"（@3638740） |

- **响应**：`data.dirQuestions`——题目数组，赋给 `this.questions`（@3638966 附近）；`data` 缺失时降级为 `[]`。
- **备注**：失败时把 `result.code`/`result.msg` 原样向上抛（@3638794），不重试。无 ut。
- **证据**：main/index.js@3567479、3558676、3638740、3638786

#### POST /client/task/linked — 任务关联目录（配置键 simpleTreeInfo）

- **域**：book-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：从"任务"反向定位它在目录树中的位置（章节链 + 任务节点），用于任务跳转与成绩回填时确定任务归属章节。
- **调用点**：`EF.getTaskLinked(bookId, taskId, type=1)` @3567779；调用方 `loadTaskLinked()` @3622829、`fetchSimpleBookTreeInfo()` @3868900
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID |
| taskId | string | 任务 ID |
| type | number | 默认 `1`，由调用方透传 |

- **响应**：`data.chapters[]`——每项含 `id`、`parentId`，据此把章节挂到已有树上（`n.data?.chapters.forEach` @3623457）。
- **备注**：**有缓存**——key=`getTaskLinked_${uid}_${bookId}_${taskId}_${type}`，TTL `72e5` ms（2 小时），仅 `result.code==0` 时写缓存（@3568020）；上层 `fetchSimpleBookTreeInfo` 另有一层 `AppInfo.getMemoryData` 内存缓存（@3868920）。无 ut。
- **证据**：main/index.js@3567779、3562305、3622829、3623457、3568020

#### GET /client/book/finished-task/list（PC）/ POST（H5） — 已完成任务分数

- **域**：dsb-api.up366.cn
- **鉴权**：TGT → 一次性 ST → `?ticket=`（见 `recon/wrongnote-h5.md` §1）
- **用途**：换设备/重装后补齐本书的任务完成分数，避免本地 `TaskScore` 表为空导致进度全丢。
- **调用点**：
  - PC：`Books.initChapter()` @3617392，`Ge.getByName("getBooksFinishedTaskList",{bookId})` → **GET**（`get` 把第二参作为 searchParams，@3575184）
  - H5：`chunk-6b733b35.js@9507`、`chunk-39edd6e0.js@17926`，`post(url,{bookId})` → **POST**
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| bookId | string | 教材 ID（PC 传 `this.Id`，H5 传页面 `bookId`） |

- **方法差异**：PC 端只发 GET，H5 只发 POST；2026-11-05 未认证实测两种方法都返回 `code=-21` JSON（不是 404/405），说明网关两条路由都存在。**两端都不传 `courseId`**；库里的可选 `course_id` 无客户端依据。
- **2026-11-05 有效账号实测**：同一 `bookId` 的 GET 与 POST 返回**完全相同**的 `taskId` 集合与字段值；额外 `courseId` 被忽略。行字段：`addDate,batchId,bookId,chapterId,firstScore,maxScore,percent,score,seconds,studyDate,taskId,uid`。
- **响应**：`data` 为数组，每项至少含 `taskId`；客户端以 `data[].taskId` 为键，**整项**作为 `taskScore` 挂到任务上（`I.taskScore=s[I.Id]` @3618176）。H5 另外用 `studyDate` 排序。`taskScore` 其余字段与本地 `TaskScore` 表结构对齐（bookId/taskId/type/pid/study/score/weight/percent/studyDate），但主进程只显式判断 `taskId`。
- **备注**：仅**非离线模式**请求（`initChapter(e=!1)`，`e` 为真时跳过）；本地已有同 `taskId` 不覆盖（`s[g.taskId]||(s[g.taskId]=g)` @3617410）。失败只写日志"加载分数出错"，不弹窗不阻断。无 ut。
- **证据**：main/index.js@3617392、3558520、3617410、3618176、3575184；H5 chunk-6b733b35.js@9507、chunk-39edd6e0.js@17926

#### GET /client/student/task/answer/detail（PC）/ POST /v2（H5） — 任务作答详情

- **域**：dsb-api.up366.cn
- **鉴权**：TGT → 一次性 ST → `?ticket=`
- **用途**：按任务取学生作答详情（试卷/标准答案/学生作答 XML），用于错题回看、作文报告导出。
- **调用点**：
  - PC：`mF.getTaskAnswerDetail(e)` @3572472（`getByName("taskAnswerDetail", e)` → GET + searchParams，@3575184）；唯一调用方 `ExportContinueWritingReport` @3768493，参数 `{courseId, bookId, taskId, uid}`。
  - H5：`chunk-6c496ba5.js@892819`（mockexam 报告页，路由 `/mockexam/:uid/:bookId/:taskId`）→ **POST** `/client/student/task/answer/detail/v2`，参数 `{uid, bookId, taskId}`（无 `courseId`）。
- **请求参数**：

| 方法 | 字段 | 说明 |
| --- | --- | --- |
| PC GET | courseId / bookId / taskId / uid | 来自 `reportInfo` 与学生行；`uid` 是学生 uid |
| H5 POST /v2 | uid / bookId / taskId | 来自路由参数；`courseId` 不是必填 |

- **响应**：PC 调用方读取 `data.paperInfo.paperData`、`data.userAnswer.markData`、`data.userAnswer.score`（@3768493 之后 `dealWithResult`）；H5 `showPaper` 读取 `data.paperInfo.{paperData,answerData,cdnPrefix}`、`data.userAnswer.{answerData,markData}`（@892912）。两版结构一致：`data.paperInfo` + `data.userAnswer`。
- **2026-11-05 有效账号实测**：v1 GET 与 v2 POST 都要求 `{courseId,uid,bookId,taskId}`；`taskId` 必须取 `contentId`（传 `jobId` 会得到空 `paperInfo`）。v2 缺 `courseId` 返回 `code=-1,msg=param_missing`。v1 返回 `data.{paperInfo,userAnswer}`；v2 额外返回 `batchId,courseName,percent,seconds,studyDate,submitDate,taskName`。`paperInfo` 典型键 `answerData,cdnPrefix,duration,paperData,score`；`userAnswer` 典型键 `answerData,batchId,markData,score`。
- **备注**：此前 recon 判“死代码/主进程无调用点”是漏搜——调用点在 @3768493；v2 路径只存在于 H5 bundle，PC 安装包只有非 v2 路径（@3561535）。两版都返回完整 XML，客户端自行拼接。
- **证据**：main/index.js@3561535、3572472、3575184、3768493；H5 chunk-6c496ba5.js@892819、@892912

#### GET /client/paper/detail — 试卷详情

- **域**：qlib-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：取整套试卷的结构化内容，客户端在"导出试卷"前用它判断是否含不支持导出的题型。
- **调用点**：`mF.getPaperDetail(paperId)` @3572388（`getByName("getPaperDetail",{paperId})`）；调用方 `TryExportPaper()` @3775999
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| paperId | string | 试卷 ID，作为 query 参数拼在 URL 上（GET） |

- **响应**：`data.paperData`——**XML 字符串**。客户端要求 `xe.isString` 通过，否则返回 `{code:-999,msg:"not data.paperData"}`；随后按 `<question_type>N</question_type>` 匹配，命中 `9/12/13/8` 即判定含匹配题/口语题、拒绝导出（@3776038）。
- **备注**：响应体是 XML 塞在 JSON 字符串字段里，主进程只做子串包含判断，不做 XML 解析。无 ut。
- **证据**：main/index.js@3572388、3557865、3775999、3776038

#### POST /client/onlinetesting/alldata — 在线测评全量数据（配置键 testAlldata）

- **域**：xot-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：打开"作业"类型测评资源前，用测试 ID 换试卷 ID，再据此下载资源包。
- **调用点**：`Ge.postByName("testAlldata", {...})` @3752236
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| testId | string | 测评/测试 ID，取自资源描述对象的 `e.contentId` |
| uid | number | 当前用户 ID（`global.CurrentUser.uid`） |

- **响应**：`data.testInfo.paperId`——**唯一被读取的字段**（@3752510），直接回写到 `e.paperId`，供下一步 `/client/paper/zipinfo` 使用。
- **备注**：无 ut；`result.code!=0` 时弹窗"获取数据失败，请重试~"并中止整条下载链路。仅 `e.type==="homework"` 时调用，`textbook` 分支不走。
- **证据**：main/index.js@3752236、3562001、3752510
- **2026-11-05 实测**：用作业的 `contentId` / `jobId` / `paperId` 作为 `testId` 均返回 `code=1,msg=参数不合法`；该接口需要真实的在线测评 `testId`，测试账号抽样作业中未找到，未做线上成功验证。

#### POST /client/paper/zipinfo — 试卷资源包信息

- **域**：xot-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：按试卷 ID 取资源包（zip）的下载地址与版本标识，是作业/试卷离线下载的取址接口。
- **调用点**：`Ge.postByName("paperZipInfo",{paperId})` @3752610；另有薄封装 `fme(paperId)` @3866090
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| paperId | string | 试卷 ID（作业链路里来自 `testAlldata` 的 `data.testInfo.paperId`） |

- **响应**：
  - `data.downloadurl`——资源包下载地址
  - `data.fileId`——资源包版本标识；写入本地 `version` 文件，下次比对相同就跳过重复下载（`!=n.data.fileId` @3752941）

- **备注**：无 ut；下载目录固定为 `${saveDir}/homework/${paperId}`。失败弹窗"获取数据失败，请重试~"。
- **证据**：main/index.js@3752610、3561940、3866090、3752941

#### POST /client/task/score/submit/v2 — 提交学习任务成绩

- **域**：study-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：把一条教材学习任务的作答与得分上报服务端（教材书页练习的最终成绩），是教材域的主成绩通道。
- **调用点**：`M2.submitTaskScore(tasksJson, submitType)` @3579119；唯一调用方 `yl.uploadScore()` @3875656
- **请求参数**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| tasksJson | string | `JSON.stringify([TaskStuded 整行])`；外层是 form 字段，值是字符串（二次序列化） |
| submitType | number \| string | 调用方传 `e.submitType \|\| "0"`，缺省是**字符串** `"0"`；方法内再以 `t ?? 0` 兜底 |
| ut | string | 防篡改签名，构造见下 |

- **ut 算法**（@3579119）：`r = md5(String(now))`；`ut = r[0:10] + md5( md5(tasksJson) + "submitTaskToken-pc-987654" + r ) + r[10:]`。`qt` 即 md5-hex（@605892），盐值 `pc-987654` 硬编码在包内。
- **响应**：只判断 `result.code`：`0` = 成功（本地置 `uploaded=2`、`failed=0`、`seconds=0`）；非 0 时按 `code===-28` 分流为"网络异常，可重试(-1)"或"业务失败，不再重试(-2)"（@3875740）。服务端错误码表见 @373950：`-21` 没有登录、`-22` ST 校验失败、`-23` 没有 TGT、`-24` 生成 ST 成功、`-25` 生成 ST 失败、`-26` 生成 TGT 成功、`-27` 生成 TGT 失败、`-28` 请求异常、`-29` 账号锁定、`-30` 账号禁用。
- **备注**：
  - **发送前改写的字段**（@3875485）：`qstJson` 由 `{"${elementId}_${order}":{...}}` 对象还原为**数组**；`extendParams` 由字符串 `JSON.parse` 成对象。其余列（`id`、`uid`、`uploaded`、`failed`、`msg`、`batchTag`）连同本地簿记字段一并上报。
  - **textbook 分支的 `result` 取值**（`submitAnswerData(e,"textbook",json)` @3793104）：`result = JSON.stringify({studentid: uid, testInfo:{answerdata: i.userAnswerXmlStr, markdatao: i.markDataXmlStr}})`（@3793860）。两份 XML 均由**渲染层**经 IPC 传入，主进程不生成、不改写、不校验；`result` 在整个 main bundle 里**只写不读**，语义完全由服务端定义。
  - **离线队列状态机**（`yl` 类 @3870329，DB 字段 `uploaded`）：`0` 待上传 → `1` 上传中（`start()` 先把该 taskId 行置 1，@3876956）→ `2` 成功；失败为 `-1`（网络，可重试）或 `-2`（永久，不再重试）。重试扫描 SQL 为 `uploaded!=2 and uploaded!=-2`，每 10s 轮询（@3878106 / @3877768）。对外状态映射 `_x={-2:-1,-1:-1,0:0,1:0,2:1}`（@3870290）。
  - 行内 `uid==0` 视为"未登录期间的本地草稿"，`getUploadInfo` 会强制认为 `uploaded=2`、不上报（@3881447）。
  - 上报成功后本地 `seconds` 立即归零（@3876390），配合 `uploadTaskScore` 里的 `e.seconds+=i.seconds` 累加逻辑避免重复计时；`seconds` 为空时兜底 `200`。
  - **请求体是 form 编码**：`Ge.post` 走 `new Pc(obj)`，默认 `application/x-www-form-urlencoded`，对象值被 `JSON.stringify`、键名中 `###` 替换为 `.`（@3536622）；`tasksJson` 本身是字符串，原样提交。
- **证据**：main/index.js@3579119、3559958、3875485、3875656、3870329、3870290、3876956、605892、373950

#### POST /client/task/score/gzip/submit — 任务成绩 gzip 提交（含附件引用）

- **域**：study-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：新版阅读器（JSSDK / 2025 版）的任务成绩上报，支持随成绩一起引用录音等附件。走离线队列表 `CommonUpload`，把"附件对象名 + 请求体"整体打包后重放。
- **调用点**：IPC 方法 `submitTask({key, urlList, tasksJson, submitType})` @3827361 → `mE` 组包 @3881783 → `dy.uploadCommon()` @3879642
- **请求参数**（`requestParams` 里的每一条请求体）：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| tasksJson | string | `JSON.stringify([e.tasksJson])`；`tasksJson.uid` 为空时先用 `global.CurrentUser.uid` 补齐（@3827480） |
| ut | string | `Ge.buildUT(tasksJson,"submitTaskToken")` @3827583；算法与 v2 通道完全一致（`md5(md5(payload)+"submitTaskToken-pc-987654"+md5(now))` 三段拼接） |
| submitType | number \| undefined | 直接取 `e.submitType`，**不兜底**，可能缺失（与 v2 通道的 `"0"` 不一致） |

- **请求头**：`Content-Type: application/json;charset=UTF-8`、`Content-Encoding: gzip`，硬编码在队列项里（@3827665）。
- **请求体**：不是 JSON 字符串，而是 `gzipSync(JSON.stringify({tasksJson,ut,submitType}))` 的**二进制 Buffer**；`Ge.post(url, buffer, {headers})` 时 `rf.send()` 对 Buffer 直接 `request.end(buffer)`，跳过 form 序列化（@3548150）。
- **响应**：`CommonUpload` 分支只判断 `result.code==0`；非 0 时把该条 request 的 `uploaded` 置 `-1`，并让整行 `uploaded` 走 `-1/-2` 判定（@3874820）。
- **备注**：
  - **附件打包方式**（`mE` @3881783）：`urlList` 中每个非空 URL 经 `new URL(u).pathname.slice(1)` 取出 **objectName**，去重后 `join(",")` 成 `objectIds`（@3882081 附近）。真正的文件上传走 `smartUploadFile` → 迷你云 / 私有云通道，**不在本接口**；本接口只用 `objectIds` 建立引用关系。
  - `CommonUpload` 表列：`taskId(PK), uuid, objectIds, requestParams, uploaded, failed, msg`；`taskId` 与 `uuid` 都等于 `mE` 的 `key`（@3881993）。`requestParams` 是请求数组的 JSON 串，每项形如 `{method,url,headers,body,gzip,uploaded}`。
  - **与 v2 通道的状态机差异**：`CommonUpload` 的扫描条件是 `uploaded!=2`，**没有排除 `-2`**，因此被判永久失败的包仍会被反复重放（扫描 @3878614）。
- **证据**：main/index.js@3827361、3827583、3827665、3560025、3881783、3881993、3874743、3879642、3548150

#### POST /client/onlinetesting/submit — 提交在线测评成绩

- **域**：xot-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：把一次在线测评（作业/试卷）的作答数据整体上报给 xot 服务端，与教材任务成绩是两条独立通道。
- **调用点**：`M2.submitTestScore(payload)` @3579452（`postByName("TestScore", e)`，纯透传）；唯一调用方 `yl.uploadScore()` 的 `else` 分支 @3875955
- **请求参数**：**整个 `TestStuded` 行原样作为请求体**，发送前只补一个 `testId`：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| testId | string | 发送时现补 `e.testId = e.taskId`（@3875945），不是 DB 列 |
| taskId | string | 测评 ID（作业分支里等于 `i.testId`） |
| paperId | string | 试卷 ID |
| uid | number | 用户 ID |
| answerData | string | **作答 XML**，作业分支来自渲染层传入的 `i.userAnswerXmlStr`（缺省 `""`，@3793300） |
| duration | number | 作答时长（秒），缺省 `1` |
| beginTime / endTime | string | 开始/提交时间，作业分支分别取 `i.startTime` / `i.submitTime`，缺省 `""` |
| playData / timeData | string | 录音与逐题耗时数据，缺省 `""` |
| studyDate | string | 本地入队时写 `""`，上报保持空 |
| uploaded / failed / msg | number / number / string | **本地队列簿记字段，也被一并上报** |

- **响应**：只判断 `result.code==0`；非 0 时按 `-28` 分流 `-1`（可重试）/ `-2`（永久失败），日志带 `taskId`、`paperId`（@3875963）。
- **备注**：
  - **本接口没有 ut 签名**（对比 `/client/task/score/submit/v2` 有 `ut`），服务端无法校验提交体完整性——这是两条通道最显著的差异。
  - 入队口 `yl.uploadTestScore(e)` @3880798 直接 `Qt.data.save("TestStuded", e)`，调用方必须给出完整行；实际唯一入口是 `submitAnswerData(e,"homework",json)` @3793104（`taskSubmited=!0` 后关闭页面会触发 `window.refreshTaskList()`）。
  - `TestStuded` 表列：`taskId(PK), paperId, uid, answerData, duration, studyDate, beginTime, endTime, playData, timeData, uploaded, failed, msg`（@3526060）。
  - 作业分支的 `answerData` 与教材分支的 `result` 内 XML 由**同一处**渲染层入参分叉产生，二者互不共用字段。
- **证据**：main/index.js@3579452、3560098、3875945、3875963、3880798、3793104、3793300、3526060

---

### 1.13 字段级详情：文件 / 消息 / 第三方 / 私有云（15 条）

#### POST https://fs.up366.cn/fileinfo/{fileId} — 单文件元信息（下载前置）
- **域**：fs.up366.cn
- **鉴权**：需要 u3t；返回 `result.code==-21/-22` 时走 ST 兜底（先打 `/v1/serviceTicket` 拿一次性票据，再把 `?ticket=` 拼到本 URL 上重放一次）
- **用途**：按 fileId 取文件的下载地址、大小、md5。课本、章节、听力、课件、资源包等一切下载都先经过它。
- **调用点**：M2.getFileInfo() @3575475；缓存包装 getFileInfoFromCache() @3575631；实际调用 Bl.start() @3593266、qF.run() @3632556、pu.getPackageSize() @3711994、pu.getDownloadSize() @3712713、pu.getBookAndChapterDownload() @3714827
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| fileId | string | **路径参数**，直接拼在 `/fileinfo/` 后；调用方先用 `/^\d+$/` 判断「是 fileId 而不是 http 地址」才进来 |
| app | string | 固定 `"school"`，写死在方法体里 |
| auth | string | HMAC-SHA1(key=`au66ssoI3nGr6DxCHk8KcpVsQ1wYNQId`, msg=`GETOBJECTschool{fileId}`) → base64 |

- **响应**：`result.code==0` 为成功；读 `data.downloadUrl`（若以 `http://` 开头，先强制替换成 `https://` 才写入缓存）、`data.filemd5`（下载完成后做整文件校验）、`data.filesize`（字符串，调用方一律 `Number()` 转换后用于进度/容量）。单文件路径不回读 `data.fileId`。
- **备注**：有缓存——结果写进 `fileInfoCacheMap`（普通 Map，无 TTL、无失效、无上限），此后同一 fileId 不再发请求；请求体是 `application/x-www-form-urlencoded`；方法内部 catch 住异常返回 `je.Error(-28, e.message)` 而不是抛出；调用方（Bl.start @3593266）在 code!=0 时打点 `DownloadErr_DownloadFailed` 并附 `extData`。
- **证据**：main/index.js@3575475（方法定义）、@3575631（缓存；http→https 改写 @3575834）、@3559734（`fsFileInfoUrl` 常量）

#### POST https://fs.up366.cn/fileinfo/list — 批量文件元信息（每批 ≤100）
- **域**：fs.up366.cn
- **鉴权**：需要 u3t；同样走 `-21/-22 → ST` 兜底
- **用途**：一次拿多份文件元信息，用于资源包/整本课本批量下载前统计总大小。
- **调用点**：M2.getFileInfoBatch() @3575966；唯一调用方 qF.run() @3637053（`Ge.getFileInfoBatch(fileIds)`）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| fileids | string | 逗号分隔的 id 串，**单次最多 100 个**：`Math.ceil(n/100)` 分批，`slice(o*100,(o+1)*100)`，末批取 `Math.min(len,(o+1)*100)` |
| app | string | 固定 `"school"` |
| auth | string | HMAC-SHA1(同密钥, msg=`GETOBJECTschool{逗号 join 的 fileids}`) → base64 |

- **响应**：`data` 是数组，元素含 `fileId`（写回缓存用的 key；调用方还按**请求顺序下标**取 `t.data[r]`，即强依赖同序返回）；`filesize` 用于累加总大小。任一批 `result.code!=0` 直接返回该错误；接口返回后若仍有 id 不在缓存，返回 `je.Error(-999,"接口找不到文件信息(ID:x)")`。
- **备注**：发请求前用 `filter(o=>!fileInfoCacheMap.has(o))` 剔除已缓存 id，全部命中时**一个请求都不发**；同样写入无过期缓存。
- **证据**：main/index.js@3575966（方法定义）、@3637053（调用点）、@3559779（`fsFileInfoUrlBatch` 常量）

#### GET https://fs.up366.cn/download — 主进程未使用的死配置
- **域**：fs.up366.cn
- **鉴权**：未接线
- **用途**：若服务端走「中转下载」本应打这里；实际下载**不经过它**，而是直接 GET `/fileinfo` 返回的 `data.downloadUrl`（OSS/CDN 直链）。
- **调用点**：无。`fsDownloadUrl` 在配置对象里定义后，全 bundle 仅此 1 次出现（@3559834）；`M2.downloadByFileId()` @3576697 也只是 `getFileInfo` + `http.download(data.downloadUrl)`，且该方法同样无调用点。
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| （无） | — | 代码中未构造任何参数 |

- **响应**：未被消费。
- **备注**：死配置 + 死方法。`downloadByFileId` 内部会把 `http://` 升级成 `https://`（@3576820），并在 `md5=true`（默认）时把 `data.filemd5` 传给下载器做校验。
- **证据**：main/index.js@3559834（常量）、@3576697（downloadByFileId）

#### POST https://msg-api.up366.cn/client/msg/list-of-pc — PC 端未读消息
- **域**：msg-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：拉取 PC 端未读消息列表（角标/消息中心数据源）。
- **调用点**：hF.getMyMsg() @3566314；pi.getMyMsg() @3894017 转发 `Ge.user.getMyMsg()`
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| status | number | 固定 `0` |
| pager.currentPage | number | 固定 `1`（form 键名保留点号，`Pc.getForm()` 只把 `###` 转成 `.`，不动已有点） |
| pager.pageSize | number | 固定 `999`，即一次拉全部未读 |

- **响应**：主进程不解构任何字段，`postByName` 的 JSON 原样返回。
- **备注**：**当前不可达**——`ie([It],pi.prototype,"getMyMsg",1)` 不在 IPC 注册表里（注册表 @3897413 起共 22 项，无 getMyMsg），主进程内也没有第二个调用点。
- **证据**：main/index.js@3566314（方法）、@3894017（IPC 转发）、@3897413（注册表）、@3557135（`findUnreadMsgList` 常量）

#### POST https://mall-api.up366.cn/client/order/goods/validity — 商品/订单有效性校验
- **域**：mall-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：按接口名推断为买书/加购前的商品有效性校验；代码中无调用方，无法从上下文确认具体业务场景。
- **调用点**：hF.getOrderGoodsValidity() @3566437（全 bundle 唯一出现）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| （无） | — | `postByName("orderGoodsValidity")` 未传第二个参数，最终发出的是**空 body 的 POST**（`send()` 中 `!this.opts.form` 直接 `request.end()`） |

- **响应**：未被消费。
- **备注**：死代码（无调用点、未注册 IPC）。同域的 `addGoods`、`flipBookActiveCode`、`activeBook` 也是死配置。
- **证据**：main/index.js@3566437（方法）、@3562447（`orderGoodsValidity` 常量）

#### GET https://pigai-api.up366.cn/client/result — 批改结果详情
- **域**：pigai-api.up366.cn
- **鉴权**：需要 u3t（GET，无 ut、无签名）
- **用途**：导出「实用文/续写」报告时，按学生 tid 逐条拉批改结果。
- **调用点**：mF.getPigaiResult() @3572311；调用方 `Ge.other.getPigaiResult(i.tid)` @3764589（实用文报告导出）、@3768426（续写报告导出）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| tid | string/number | query 参数，来自报告数据里每个学生的 `tid`（续写流程取 `qstList[0].tid`） |

- **响应**：`result.code==0` 且 `data` 必须是对象、`data.gecResult` 必须是数组，否则整份报告导出中止并回 `获取批改数据失败 name:x code:y msg:z`；条目内字段由导出模板消费，主进程只做上述两个判断。
- **备注**：`getByName` 走 GET + searchParams；配置里另有 `getPigaiResult=".../client/batch/result"`（@3561054）从未使用，是死配置（批量版被单条版取代）。
- **证据**：main/index.js@3572311（方法）、@3764589 / @3768426（调用点）、@3561422（`pigaiResult` 常量）

#### POST https://livefile-api.up366.cn/client/classfile/upload — 直播课件（PPT）上传
- **域**：livefile-api.up366.cn
- **鉴权**：需要 u3t
- **用途**：投屏/直播场景把本地 PPT 传到直播文件服务，供教师端与学生端同屏使用。
- **调用点**：EF.liveFileUpload() @3567592（组装 multipart）；实际调用 main-window.js uploadPPTFile() @4236501
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| fileMD5 | string(text) | 本地文件的 md5 十六进制值，由 `La()`→`Pp()` 分块（256/512/1024 KB）读盘计算 |
| fileName | string(text) | `path.basename(本地路径)` |
| file | file(multipart) | `createReadStream(路径,{autoClose:true})` 文件流 |

- **响应**：main-window 侧 `result.code==0` 时把 `data` 整体展开进 `pptfileupload` 事件（`uploadStatus:1, uploadProgres:100, ...data`）；失败回 `uploadStatus:-1 + result.msg`。`data` 内部字段未显式读取，属透传。
- **备注**：`Pc.setFile()` 会把 contentType 切成 `multipart/form-data`，boundary 由 `rf.send()` 补进 Content-Type；上传走 `uploadQueue`（并发上限 10）并带 `uploadProgress:true`，每 500ms 抛一次 `upload-progress`；大小上限来自渲染层传入的 `e.maxFileSize`（MB），超限直接回 `uploadStatus:-3`，**不发请求**。
- **证据**：main/index.js@3567592（方法）、@3561298（`livefileUpload` 常量）、@3536357（Pc 表单类）；main-window.js@4236501（调用点）

#### POST https://fy.ismartin.com/essay/assess/api — 作文批改（第三方）
- **域**：fy.ismartin.com
- **鉴权**：随统一传输层带上 u3t/Authorization 等 7 个头，但第三方不校验；服务端若回 `-21/-22` 同样会触发 ST 兜底（把 `?ticket=` 拼到第三方 URL 上）
- **用途**：课文/作业页的「作文批改」，把学生作文与参考范文送第三方评分。
- **调用点**：main-window.js pi.doCorrect() @4279997 → `Ie.postByName("essayCorrect",{essay,ref})` @4280286
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| essay | string | = 渲染层传入 JSON 的 `cont` 字段（作文正文） |
| ref | string | = 渲染层传入 JSON 的 `refrence` 字段（参考范文） |

- **响应**：`result.code==0` → 把 `data` 原样注入渲染层 `window.onDoneCorrect(id, data)`；否则回调 `{reqError:"请求失败！"}`。
- **备注**：入参是渲染层传来的 JSON **字符串**，主进程 `JSON.parse` 后只校验 `cont`/`refrence` 是否存在，缺一个就回调错误且不发请求；URL 不含 `up366.cn`，不会被平台域名 Proxy 改写，因此测试/UAT 环境仍打生产第三方。
- **证据**：main-window.js@4279997（doCorrect）、@4280286（请求）；main/index.js@3558818（`essayCorrect` 常量）

#### POST https://fanyi.ismartlearning.cn/correct — 作文纠正（第三方）
- **域**：fanyi.ismartlearning.cn
- **鉴权**：同 essay 接口，带统一 7 头；`-21/-22` 会触发 ST 兜底
- **用途**：作文「纠错」通道，与上一接口并列的第二种第三方作文服务。
- **调用点**：main-window.js pi.doCompositionCorrect() @4280618 → `Ie.postByName("compositionCorrect",{comcontext:t})` @4280684
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| comcontext | string | 渲染层传入的原始字符串，**未做 JSON.parse**，原样透传 |

- **响应**：`result.code==0` → `window.onDoneCompositionCorrect(id, data)`；否则 `{reqError:"请求失败！"}`。
- **备注**：与 essay 接口相比没有入参校验；失败日志也复用 `"doCorrect 失败"` 文案。
- **证据**：main-window.js@4280618（方法）、@4280684（请求）；main/index.js@3558874（`compositionCorrect` 常量）

#### POST https://hw-rpt-cloud-api.{platform}/client/teacher/test/score-sheet?testId=123 — 云批改域名能力探针
- **域**：hw-rpt-cloud-api.{platform}（platform 为当前环境域名，如 `up366.cn`）
- **鉴权**：需要 u3t（走 `Ge.post`，即统一传输层）
- **用途**：在 `rf.start()` 里，若本次请求 URL 含 `hw-rpt-cloud-api.{platform}/client/` 且本机 1 小时内没探过，先打一枪，`result.code==1` 才记时间戳，用于判断该环境有没有云批改能力。
- **调用点**：rf.start() @3549068（传输层内部）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| testId | number | query 参数，**写死 123**（不存在的题号，故意触发拒绝以探测能力） |

- **响应**：只读 `result.code`，`===1` 视为该域名可用。
- **备注**：缓存 key 是 `hw-rpt-cloud-api.{platform}/client/`，存 `global.CurrentUser.cacheData`（Map），TTL `36e5`=1 小时（@3549244）。**实际从未执行**：三个 bundle 里除探针自身外没有任何 URL 命中该前缀；若命中，探针请求自身也含该前缀，会自递归。
- **证据**：main/index.js@3549068（判断与探测）、@3549244（`Date.now()-i>36e5` 与 `cacheData.set`）

#### GET http://{miniCloudIP}:{miniCloudPort}/minicloud/status?type=simple — 私有云存活与授权状态
- **域**：私有云 `http://{miniCloudIP}:{miniCloudPort}`（明文 http，IP/端口来自 electron-store 的 `cloud.config`）
- **鉴权**：匿名——用的是裸 `new vC` 传输实例，**不带** u3t/X-App-Name 等 7 个头
- **用途**：每 10s 轮询一次私有云状态，映射成本地 `status` 并通过 `cloudStatus` 事件推给界面；`status==4` 是「私有云可用」的唯一判据（决定上传与版本清单是否改走私有云）。
- **调用点**：_L.checkStatus(e=0,t=0) @3535768；runCheckStatus() @3535671 用 `setTimeout(...,1e4)` 自递归
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| type | string | 固定 `"simple"`（写进 URL 常量） |
| connectTimeout | number | 可选，作为 `vC.get` 第三参的 `connectTimeout`；仅 `e>0&&t>0` 时传 |
| timeout | number | 可选，同上；`checkMinicloud` 传 `(5000,10000)` |

- **响应**：`code===0` 时读 `stayOnTop`（写回 store）与 `status`：`0→2`（私有云未注册）、`1→4`（在线）、`2→3`（已过期）；请求异常或 `code!==0` → `1`（连接异常）。仅状态变化时 emit。
- **备注**：未配置 IP 或端口时 `cloudEnabled=false`，URL getter 返回空串且**完全不发请求**；pi.checkMinicloud @3896440 在状态查询前还会先用 `probe(er.miniCloudIP)` 做一次局域网探活（不通直接提示「教师机学生机内网不通，请联系机房老师」）。
- **证据**：main/index.js@3535768（checkStatus）、@3535671（10s 轮询）、@3534075（`miniCloudStatusUrl` getter）、@3896440（checkMinicloud 文案映射）

#### POST http://{miniCloudIP}:{miniCloudPort}/minicloud/upload — 私有云上传中转
- **域**：私有云 `http://{miniCloudIP}:{miniCloudPort}`
- **鉴权**：需要 u3t（走 `M2.upload` → 统一 http 实例，7 个头都在）
- **用途**：私有云在线（`cloudEnabled && status==4`）时，把本该传公网 OSS 的文件改投局域网私有云。
- **调用点**：M2.uploadFile2PriCloud() @3577735（`this.upload(er.miniCloudUploadUrl, n)` @3578310）；smartUploadFile() @3578354 在 `r && cloudEnabled && status==4` 时优先走它
- **请求参数**（multipart，由 `Pc` 组装、`setFile` 自动切 multipart）：

| 字段 | 类型 | 说明 |
|---|---|---|
| bucketName | string | 来自 STS 的 `ossBucketInfo.bucketname`，不是本地写死 |
| endpoint | string | 来自 STS 的 `ossBucketInfo.defaulthostid` |
| objectName | string | 对象键，由 `da.buildOSSObjectName(type, filename)` 生成：`storage/{userType}/{bizType}/{ext}/{yyyy/mm/dd}/{uid}[/{r}]/{filename}` |
| fileData | file | 本地文件流 `setFile("fileData", 路径)` |

- **响应**：`.json()` 原样返回；调用方只判 `result.code`（`-28` 或其它非 0 → 上传失败，界面提示「上传失败，网络异常！」）。
- **备注**：**桶配置来源**——`da.getOSSInfo(bucketId)` → `Ge.getOssTicket()`（`/front/aliyunoss/token/v2?organId={schoolId}`）取 STS，结果按 `ossBucketInfo.currentTime` 在 electron-store `biz` 里缓存 **60s**；bucketId 由调用方给，默认 `DefaultBucketId="up366"`，无桶时 `DefaultNoBucketId="default"`。**两条上传路径**：① 私有云中转（本接口）；② OSS 直传——`da.upload(hostUrl)` 用同源 STS 走 ali-oss `multipartUpload(objectName,file,{parallel:1,timeout:1800000})`，失败重试 3 次，host 由 `https://{bucketname}.{defaulthostid}/{objectName}` 组成。是否走①只由 `cloudEnabled && status==4` 决定，`disableMiniCloud` 只影响下载。
- **证据**：main/index.js@3577735（uploadFile2PriCloud）、@3578354（smartUploadFile 分流）、@3553970（getOSSInfo/getOssHost 的 STS 字段）、@3555880（multipartUpload 选项）、@3554267（host 拼接）

#### GET http://{miniCloudIP}:{miniCloudPort}/minicloud/download — 私有云下载中转
- **域**：私有云 `http://{miniCloudIP}:{miniCloudPort}`
- **鉴权**：需要 u3t（走 `M2.http.download`，沿用同一 http 实例的 7 个头）
- **用途**：启用私有云后，所有下载（课本、章节、资源包、图片）自动改写成「私有云代取」，由局域网服务器回源公网并缓存。
- **调用点**：vC.download() @3550778（`miniCloudDownloadUrl: r?.disableMiniCloud ? void 0 : er.miniCloudDownloadUrl`）、vC.downloadForIdle() @3551044、rf.createRequest() @3540232（实际拼 URL）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| url | string(query) | `encodeURIComponent(原始下载地址)`；原始地址已带签名 query，整体再编码一次 |
| filemd5 | string(query) | 追加在**原始地址**上（不是外层），仅当 `opts.md5` 存在时 |

- **响应**：透传原文件字节流。状态码特判：`206`（@3541916）解析 `content-range` 走断点续传；`200` 重置 pos；`416`（@3542331）删残文件报错；`900`（@3542598）→ 视为「等待私有云下载」，等 **2s** 重发；`909`（@3542758）→ 等 **3s** 重发；其它非 2xx 直接报错且**不做回源兜底**（`opts.miniCloudDownloadUrl` 存在时跳过 `localBackDownload`）。
- **备注**：回源兜底 `localBackDownload()`：给原始 URL 追加 `back=1` 后重发；再次调用时把 `back` 换成 `local=1`；已含 `local` 或未开 `useLocalBackDownload` 则返回 false 不再兜底。`disableMiniCloud:true` 的下载（`Bl` 构造入参）不走中转，直连公网。
- **证据**：main/index.js@3550778（注入中转地址）、@3540232（`?url=` 拼接与 `filemd5`）、@3541916 / @3542331 / @3542598 / @3542758（206/416/900/909 分支）、@3539457（`isMiniCloudUrl` 正则 `^http://\d+.\d+.\d+.\d+:\d+/minicloud/`）、@3539917（`localBackDownload`）

#### GET http://{miniCloudIP}:{miniCloudPort}/minicloud/update/list — 私有云版 U3 子应用版本清单
- **域**：私有云 `http://{miniCloudIP}:{miniCloudPort}`
- **鉴权**：需要 u3t
- **用途**：私有云环境下代替公网 `/front/app/version/list`，给出各 U3 子应用在私有云上的最新版本，驱动子应用热更新。
- **调用点**：mF.getUpdateConfigList() @3572097；main-window.js U3AppUpdate.checkUpdateList() @4164421 → `Ie.other.getUpdateConfigList(i,t)` @4164678
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| modelJson | string(query) | `JSON.stringify([{modelName, versionNo}])`，versionNo 为本地版本 |
| username | string(query) | 当前登录用户名 |

- **响应**：`result.code==0` 且 `data` 为数组；调用方按 `data[].modelName` 建索引，取 `data[].lastVersion` 交给 `checkUpdateFromInfo`；某 modelName 查不到时回退 `{status:0}`（视为无更新）。
- **备注**：仅在 `cloudEnabled` 时走本接口，否则同参打公网 `checkVersionList`；GET query，无 ut/签名。
- **证据**：main/index.js@3572097（分流）、@3534184（`miniCloudUpdateList` getter）；main-window.js@4164421 / @4164678（调用与字段消费）

#### GET http://{miniCloudIP}:{miniCloudPort}/minicloud/update?model=STUDENT-PC-MINICLOUD-UPDATE — 私有云更新信息
- **域**：私有云 `http://{miniCloudIP}:{miniCloudPort}`
- **鉴权**：需要 u3t（走全局 `Ge.get`）
- **用途**：按接口名看是取私有云自身（服务端组件）的更新信息。
- **调用点**：_L.miniCloudUpdateInfo() @3535266（全 bundle 唯一出现，无调用方）
- **请求参数**：

| 字段 | 类型 | 说明 |
|---|---|---|
| model | string(query) | 写死 `STUDENT-PC-MINICLOUD-UPDATE` |

- **响应**：未被消费（直接 `return Ge.get(...)`）；未配置私有云时提前返回 `je.Error(-1,"未配置私有云地址!")`。
- **备注**：死代码——主进程无调用点，IPC 注册表也未暴露。`miniCloudUpdateList`（update/list）与它是两个不同接口，别混。
- **证据**：main/index.js@3535266（方法）、@3534075 附近同类 getter 结构

---

### 1.14 答题 / 考试提交链路（重点）

提交有三种入口，最终落到两个服务端接口 + 本地三张队列表。

#### 1.14.1 入口一览

| 入口 | 桥接方法 | 形态 | 目标接口 |
|---|---|---|---|
| 页面单次提交 | `jsBridgeObj.submitAnswerData(type, _, jsonStr)` | JSON 字符串 | 先写本地队列，再由队列上传 |
| 批量提交（带附件） | `submitTask({key, urlList, tasksJson, submitType})` | zip + gzip | `/client/task/score/gzip/submit` |
| 启动补交 | `startUpload()` 扫表 | 表驱动 | 同下两个接口 |

`submitAnswerData` 的分支只有两个（main-window.js@4051232）：

```js
async submitAnswerData(e, t, i){          // t = "homework" | "textbook"
  let A = JSON.parse(i);
  switch(t){
    case "homework":  // 在线测试/作业（整卷）
      uploadTestScore({ uid, taskId:A.testId, paperId:A.paperId,
        answerData:A.userAnswerXmlStr ?? "", duration:A.duration ?? 1,
        beginTime:A.startTime ?? "", endTime:A.submitTime ?? "",
        playData:A.playData ?? "", timeData:A.timeData ?? "", uploaded:0, ... })
    case "textbook":  // 课本任务（按题）
      uploadTaskScore({ id, uid, bookId:A.bookId, taskId: n?.practiceId ?? A.taskId,
        chapterId:A.chapterId, taskNo:A.taskNum, score:A.score, percent:Number(A.percent),
        studyDate: now, seconds:A.duration,
        result: JSON.stringify({ studentid: uid, testInfo:{
          answerdata: A.userAnswerXmlStr, markdatao: A.markDataXmlStr } }),
        srcType:2, batchTag: uuid, uploaded:0, studyTaskId:A.studyTaskId,
        courseId:String(A.courseId), extendParams:..., qstJson:"", msg:"", failed:0,
        killStudyTask:A.killStudyTask, submitType:A.submitType })
  }
  this.uploader?.start();
}
```

渲染层只需要给一个 JSON 字符串，含 `userAnswerXmlStr`（作答 XML）与 `markDataXmlStr`（批注 XML）；二者在 textbook 分支被包成
`{"studentid":"<uid>","testInfo":{"answerdata":"<xml>","markdatao":"<xml>"}}` 再 `JSON.stringify` 成 `result` 字段。

#### 1.14.2 本地队列（断网/崩溃后补交）

| 表 | 用途 | 关键列 |
|---|---|---|
| `TaskStuded` | 课本任务待交 | id, taskId, bookId, uid, chapterId, taskNo, score, percent, studyDate, seconds, result, srcType, batchTag, uploaded, studyTaskId, courseId, extendParams, qstJson, msg, failed, killStudyTask, submitType |
| `TestStuded` | 在线测试待交 | uid, taskId, paperId, answerData, duration, beginTime, endTime, playData, timeData, uploaded, studyDate, failed, msg |
| `CommonUpload` | 通用文件待传 | id, taskId, objectIds, uuid, uploaded |
| `RecordFiles` | 录音文件 | id, taskId, pageId, recordId, recordFile, objectName, fileSize, uploaded, msg |
| `StudyTaskRecord` | 学习记录 | taskId, studyTaskId, score, studyDate |

`uploaded` 状态机：`0` 未传 → `1` 上传中 → `2` 成功 → `-1` 可重试失败 → `-2` 不可重试失败。
返回值 `-28`（请求异常）→ `-1` 保留重试；其它非 0 → `-2` 判定为业务拒绝，停止重试（main/index.js@3875300 附近）。

#### 1.14.3 `qstJson` —— 按题粒度的作答（真正的"题目提交"）

课本任务是**按题**存的，不是整卷：

```js
// addTaskStuding(bookId, taskId, list)  main-window.js@4136800 附近
let n = {};                       // 已有则复用
for (const s of list) n[`${s.elementId}_${s.order}`] = s;   // 键 = 题元素ID_序号
A.qstJson = JSON.stringify(n);
ct.data.save("TaskStuded", A);
```

- 键：`<elementId>_<order>`，值：该题的作答对象（含分数等业务字段）。
- 上传前被"拍平"成数组（main/index.js@3875450）：`Object.entries(qstJson).map(x => x[1])` —— **键名在服务端是无意义的**，顺序按对象插入序。
- `uploadTaskScore` 会先做一次合并：新作答覆盖旧 `qstJson` 同名键，并把 `seconds` 累加到历史值上（`e.seconds += A.seconds`，为 0 时兜底 200 秒）。

#### 1.14.4 线上接口

**① 课本任务成绩**

```
POST https://study-api.up366.cn/client/task/score/submit/v2
Content-Type: application/x-www-form-urlencoded

tasksJson = JSON.stringify([ row ])   // 注意：是"数组的 JSON 字符串"，外层再当表单值
submitType = 0 | "0" | ...
ut         = md5(T).slice(0,10) + md5( md5(tasksJson) + "submitTaskToken-pc-987654" + md5(T) ) + md5(T).slice(10)
```

- 调用：`Ge.submitTaskScore(tasksJson, submitType)`（main/index.js@3579125）。
- `submitType` 为空时默认 `0`；离线队列传的是 `e.submitType || "0"`（字符串）。
- 单次一行；批量走下面的 gzip 接口。

**② 在线测试成绩**

```
POST https://xot-api.up366.cn/client/onlinetesting/submit
Content-Type: application/x-www-form-urlencoded

uid, taskId(=testId), paperId, answerData, duration, beginTime, endTime,
playData, timeData, uploaded, studyDate, failed, msg
```

- 调用：`Ge.submitTestScore(row)`（main/index.js@3579458），**整行对象直接当表单提交**，没有 `ut`、没有签名。

**③ 批量 / 带附件（gzip）**

```
POST https://study-api.up366.cn/client/task/score/gzip/submit
Content-Type: application/json;charset=UTF-8
Content-Encoding: gzip

{ tasksJson: JSON.stringify([task]), ut, submitType }
```

- 调用：`submitTask({key, urlList, tasksJson, submitType})`（main-window.js@3827590 附近）。
- `urlList` 里的附件先打进一个 zip（`mE` 类，7za 或内置实现），再整体 gzip。
- `ut` 用 `buildUT(tasksJson, "submitTaskToken")`，与 ① 的盐一致（`-pc-987654`）。

**④ `ut` 的可复现性（与前面结论一致）**

```js
md5(T).slice(0,10) + md5( md5(tasksJson) + "submitTaskToken-pc-987654" + md5(T) ) + md5(T).slice(10)
```
常量、盐、拼接顺序全部在 bundle 里，**离线可完整构造**；`T=md5(Date.now())` 不可逆 ⇒ 服务端无法施加时效窗口。

#### 1.14.5 一张图

```
渲染层 (提交按钮)
  └─ jsBridgeObj.submitAnswerData("textbook", _, JSON{userAnswerXmlStr, markDataXmlStr, ...})
       └─ uploadTaskScore()  → SQLite TaskStuded（qstJson 按题存储）
            └─ df.uploader.start()
                 └─ POST /client/task/score/submit/v2   {tasksJson, submitType, ut}
                      └─ 失败: uploaded=-1(可重试) / -2(放弃)
                      └─ 下次启动 startUpload() 扫表重放
```

---

#### 1.14.6 六个容易踩的坑（实测）

1. **两条成绩通道不对称**：`/client/task/score/submit/v2` 带 `ut`；`/client/onlinetesting/submit` **完全没有 ut**，整行对象透传。
2. **`submitType` 类型不一致**：v2 传 `e.submitType || "0"`（字符串兜底），gzip 通道原样传（可能是 `undefined`）。
3. **本地簿记字段被一起上报**：`id` / `batchTag` / `uploaded` / `failed` / `msg` 这些纯本地状态也会进服务端请求体。
4. **`CommonUpload` 会无限重放**：`TaskStuded` / `TestStuded` 的重试扫描会跳过 `uploaded == -2`，但 `CommonUpload` 的扫描条件只有 `uploaded != 2`，永久失败的包每 10s 重放一次（main-window.js@3878614 附近）。这是客户端侧的真实缺陷。
5. **`seconds` 上报后立即归零**（main/index.js@3876390 附近），配合 `e.seconds += 旧值` 累加，避免重启后重复计时；为 0 时兜底 `200`。
6. **`u3t` 存的是 TGT 不是 ST**；ST 只在 `result.code ∈ {-21,-22}` 时临时取一次并拼 `?ticket=` 重放。`Authorization` 头全包无赋值点，是死头。

另外一处死代码已在 §1.13 标注：`/download`（只用 `downloadUrl`，方法 `downloadByFileId` 无调用点）。
`mF.getTaskAnswerDetail` 原先被判无调用点，2026-11-05 已推翻：`ExportContinueWritingReport` @3768493 调用它，参数 `{courseId,bookId,taskId,uid}`（见 `recon/wrongnote-h5.md`）。

---

### 1.15 统计与口径修正

**原始统计（URL 常量 + 表面调用点）**

- `grep -o 'up366\.cn' | wc -l` = **100**（main/index.js）/ 100（main-window.js）/ 100（preload/index.js）。
- 去重后 up366 域内绝对 URL：**95 条**（24 个 host）。
- 其中主进程实际发起请求的 up366 域 API：**46 条**（其余为纯网页地址或死配置）。
- 非 up366 域：**5 条**（fy.ismartin、fanyi.ismartlearning、hw-rpt-cloud-api、微信/QQ OAuth 各 1）+ 私有云 **5 条**。
- **合计 56 条**；另有死配置（有 URL 常量、零调用点）**21 条**。

**字段级复核后证伪的 4 条**

| # | endpoint | 证伪理由 |
|---|---|---|
| 1 | `POST fs.up366.cn/download` | 下载只用 `/fileinfo` 返回的 `data.downloadUrl`；`downloadByFileId` 全 bundle 无调用点 |
| 2 | `POST msg-api.up366.cn/client/msg/list-of-pc` | 包装方法 `pi.getMyMsg()` 不在 IPC 注册表，主进程无其它调用点 → 渲染层够不到 |
| 3 | `POST mall-api.up366.cn/client/order/goods/validity` | 主进程无任何调用点 |
| 4 | `GET /minicloud/update?model=…` | 无调用点 |

> 2026-11-05 更正：原第 4 条 `GET /client/student/task/answer/detail` 不是死代码——`ExportContinueWritingReport` @3768493 调用它，
> 参数 `{courseId,bookId,taskId,uid}`；同功能的 H5 `POST /v2` 见 `recon/wrongnote-h5.md`。该条从证伪表移除。

另：`GET /front/health-status` 有 URL、有 `MainWindow.checkOnline()` 包装，但**未找到调用方**，存疑。

**修正后口径**：主进程实际可发起的 endpoint ≈ **52 条**（up366 43 + 外部 5 + 私有云 4）。
本文档字段级覆盖 **54 条**（§1.11 认证配置 22 / §1.12 教材任务评测 17 / §1.13 文件及其它 15），上述 4 条保留并标注为死代码，便于版本比对。

---

## 2. 统一请求封装

### 2.1 调用栈

```
业务 (CF.login / hF.user / EF.flipbook / mF.other / 各 IPC handler)
  → M2.post/get/postByName/getByName        (main/index.js:3573380)   ← 统一错误处理 + ST 重试
    → vC.post/get/download/upload           (main/index.js:3550202)   ← 只往 options 里塞 form/searchParams/method
      → rf (new)                            (main/index.js:3539318)   ← net.request 实例，重试/重定向/断点续传/md5 校验
```

### 2.2 关键源码（保留原变量名）

**传输层构造 + 全局请求头**（`main/index.js:3573380`，类 `M2`，全局单例 `Ge = new M2(ts())`）：

```js
var M2=class{http;config;platform;login;user;flipbook;other;
  loginUsername="";loginPassword="";token="";openId="";tgt="";authorization="";
  downloadQueue=new vQ(10);uploadQueue=new vQ(10);fileInfoCacheMap=new Map;
  constructor(e){
    this.platform=e,
    this.http=new vC({retry:2}),
    this.http.on("request-created",t=>{
      t.setHeader("user-agent","PC-Up366-Student 6.11.0"),
      t.setHeader("x-requested-with","PC"),
      t.setHeader("X-App-Name","student-pc"),
      t.setHeader("clientId",global.ComputerInfo?.MachineCode??" "),
      t.setHeader("u3r",pA()),
      t.setHeader("u3t",this.tgt),
      t.setHeader("Authorization",this.authorization)
    }),
    this.config=new dF(e),this.login=new CF(this),this.user=new hF(this),...
  }
```

**统一 post / get（含 ST 自动续期）**（同处）：

```js
async post(e,t,r){try{
  let i=await this.http.post(e,t,r).json();
  if(i?.result?.code===-21||i?.result?.code===-22){
    let n=await this.login.getServiceTicket(e,!0);
    if(n.result.code!==-24)return n;
    e.includes("?")?e=`${e}&ticket=${n.data?.serverTicket}`:e=`${e}?ticket=${n.data?.serverTicket}`;
    i=await this.http.post(e,t,r).json()
  }
  return i
}catch(i){oe.Inst.error("请求数据出错",e,i);let n=je.Error(-28,i.message);
  return n.result.err={code:i.code,message:i.message,name:i.name},n}}
async postByName(e,t){return this.post(this.config[e],t)}
async get(e,t,r){ ... 同上，-21/-22 → getServiceTicket → 追加 ?ticket= ... }
async getByName(e,t){return this.get(this.config[e],t)}
```

**底层发请求**（`main/index.js:3539318`，类 `rf`，关键在于 `net.request` + 手工 header）：

```js
createRequest(){
  let e=new URL(this.url);
  if(this.opts.searchParams)for(let s in this.opts.searchParams)e.searchParams.set(s,this.opts.searchParams[s]);
  let t=e.toString();
  this.opts.miniCloudDownloadUrl&&(this.opts.md5&&e.searchParams.append("filemd5",this.opts.md5),
    t=`${this.opts.miniCloudDownloadUrl}?url=${encodeURIComponent(t.toString())}`);
  let r=PD.net.request({method:this.opts.method||"GET",url:t,session:this.opts.session??PD.session.defaultSession,useSessionCookies:!0});
  this.http.emit("request-created",r);
  if(this.opts.headers)for(let s in this.opts.headers)this.opts.headers[s]!==void 0&&r.setHeader(s,this.opts.headers[s]);
  ...
}
```

### 2.3 请求头清单（全部，`setHeader` 字面量统计）

| Header | 值 | 说明 |
|---|---|---|
| `user-agent` | `PC-Up366-Student 6.11.0` | 硬编码版本号（与 `appVersion` 常量一致） |
| `x-requested-with` | `PC` | |
| `X-App-Name` | `student-pc` | |
| `clientId` | `global.ComputerInfo?.MachineCode ?? " "` | 设备指纹（主板/BIOS/OSVersion 组合，由 `AppInfo.getComputerInfo()` 采集） |
| `u3r` | `pA()` = `randomUUID().replace(/-/g,"")` | **每个请求一个新随机 UUID**（无状态、无法校验，纯链路追踪/防缓存） |
| `u3t` | `this.tgt` | 登录返回的 TGT；未登录时为空串 |
| `Authorization` | `this.authorization` | **恒为空串**：全 bundle 内除 `clearLoginData(){...this.authorization=""}` 外无任何赋值点 |
| `Content-Type` | `application/x-www-form-urlencoded` / `multipart/form-data; boundary=...` / `application/json;charset=UTF-8`（仅 gzip 成绩提交） | 由 `Pc` 决定 |
| `Content-Encoding` | `gzip` | 仅 `TaskScoreGZip` |
| `Cache-Control` | `no-cache` | `disableCache:true` 默认开启 |
| `Range` | `bytes=<size>-` | 断点续传下载 |

**没有** `timestamp` / `nonce` / `sign` / `appKey` / `platform` / `version` 头。

### 2.4 请求体编码（`Pc`，`main/index.js:3536357`）

```js
var Pc=class{contentType;form;json="";
  constructor(e={},t="application/x-www-form-urlencoded"){this.contentType=t,this.form=e}
  getForm(){switch(this.contentType){
    case"application/json":return this.json;
    case"multipart/form-data":{let e=new Q$.default;for(let t in this.form)e.append(t,this.form[t]);return e}
    default:{let e=Object.entries(this.form||{}).reduce((t,r)=>(t[r[0].replace(/###/g,".")]=xe.isObjectLike(r[1])?JSON.stringify(r[1]):r[1],t),{});
      return new B$.default.URLSearchParams(e).toString()}}}
  setFile(e,t){this.contentType="multipart/form-data",this.form[e]=...createReadStream(t)...}
  setText(e,t){this.form[e]=t}
  setJson(e){this.contentType="application/json",...this.json=JSON.stringify(e)}}
```

要点：
- **默认 form-urlencoded**；对象/数组字段会被 `JSON.stringify` 后作为单个表单值（不是嵌套 form）。
- 键名里的 `###` 会被替换成 `.`（保留服务端 `a.b.c` 扁平键风格）。
- 服务端契约：几乎所有接口返回 `{result:{code,msg}, data}`。
- **无请求体加密、无请求体压缩**（唯一 gzip 是为了绕开大 payload 上传体积，见 §1.5）。唯一与"加密"沾边的是 `utils/libup366.js` 与 `main/index.js:606089` 的本地文件 AES-128-CBC（key 硬编码 `QJBNiBmV55PDrewyne3GsA==`，全零 IV/前置 16 字节随机 IV），与 HTTP 无关。

---

## 3. 签名 / 摘要算法

### 3.1 基础原语

```js
// main/index.js:605892
function qt(A){return (0,Tu.createHash)("md5").update(A).digest("hex")}
// main/index.js:605962
function cL(A,e){return (0,Tu.createHmac)("sha1",e).update(A).digest("base64")}
```

### 3.2 `ut` 字段（唯一的"签名"，按接口各自拼串）

统一外壳：`ut = T.slice(0,10) + md5( payload + KW + T ) + T.slice(10)`，其中 `T = md5(Date.now().toString())`（32 位 hex，取前 10 / 后 22 拼接，看起来像时间戳但**不可还原**）。

| 接口 | 关键词 KW | payload | 证据 |
|---|---|---|---|
| `/v2/tickets-v3` 密码登录 | `loginEncryptionParameter` | `username + passwordHash`，其中 `passwordHash = md5( md5(明文密码) + "fa&s*l%$k!fq$k!ld@fjlk" )` | main/index.js:3563553 |
| `/v2/auto/tickets` token 登录 | `loginEncryptionParameter` | `token` 原值 | main/index.js:3563988 |
| `/v2/weChat/tickets`、`/v2/qq/tickets` | `loginEncryptionParameter` | `openId` | main/index.js:3564818 / 3565213 |
| `/front/user/verify/send` 短信 | `sendSMSToken` | `mobile` | main/index.js:3562916 |
| `getUserAPIAccountUT(mobile)`（暴露给渲染层） | `_!@#usertoken!@#_` | `mobile` | main/index.js:3563065 |
| `/client/task/score/submit/v2` | `submitTaskToken-pc-987654`（字面量含盐） | `md5(tasksJson)`（先对业务串取一次 md5） | main/index.js:3579119 |
| `/client/task/score/gzip/submit` | `submitTaskToken` + 字面量 `-pc-987654` | 同上，复用 `buildUT(i,"submitTaskToken")` | main/index.js:3579329 / 3827669 |
| 日志上传 token | 常量 `5223C@94E3!E65$942C%9D1A#062` | `md5("0")` 前缀 + 该常量 + `T`，末尾再加字面量 `0` | main/index.js:3583633 |

> 结论：**"签名"只是收发双方共享的常量关键词 + md5 混淆**，没有密钥协商、没有防重放（`T` 由客户端本地时间生成且不可验证），任何人拿到 bundle 即可完整重放。真正的鉴权强度来自 `u3t`(TGT) 与 `?ticket=`(ST)。

### 3.2.1 为什么 `ut` 不构成服务端可验证的签名（含一条可复现路径）

服务端若想校验 `ut`，只能拿收到的明文参数重算——但它**拿不到客户端的时间戳**：`T = md5(String(Date.now()))` 不可逆，服务端无法从中还原时间，也就**无法施加任何时效窗口**。因此 `ut` 的唯一作用是让"非官方客户端"多写十几行 JS，不构成鉴权。

**具体路径（短信发送接口，登录前即可调用，无 TGT 依赖）**：
```
GET https://user-api.up366.cn/front/user/verify/send?checkExists=2&mobile=<手机号>&ut=<离线计算>
```
`ut` 计算（无任何密钥，常量已在 §3.2 给出）：
```js
const md5 = s => crypto.createHash("md5").update(s).digest("hex");
const T = md5(String(Date.now()));
const ut = T.slice(0,10) + md5(mobile + "sendSMSToken" + T) + T.slice(10);
```
调用点在登录页（`sendVerifyCode(e,t)` → `t="2"`，main/index.js:3562916），**请求不携带 `u3t`**，说明该接口在服务端是匿名接口。

> **标注为推断（未验证）**：我按要求未对真实服务器发包，因此"服务端是否另有 IP/频率/图形验证码风控"未验证。客户端侧确实**没有任何**验证码、设备证明或 PoC 校验——只有上述 `ut`。

同理 `getUserAPIAccountUT(mobile)` 被暴露到渲染层（main/index.js:3563065，且在白名单内），意味着"给任意手机号生成合法 ut"这件事本身就是产品内置能力。

### 3.3 HMAC-SHA1（fs.up366.cn 文件元信息）

```js
// getFileInfo / getFileInfoBatch, main/index.js:3575521 / 3576249
auth = cL(`GETOBJECTschool${fileId}`, "au66ssoI3nGr6DxCHk8KcpVsQ1wYNQId")   // base64
body = { app:"school", auth }                                              // 单条
body = { fileids:"id1,id2,...", app:"school", auth }                       // 批量，≤100/次
```
- 消息串格式：`"GETOBJECT" + app + fileIds`（app 固定 `school`；批量时 ids 用逗号连接）。
- 密钥硬编码在客户端，等同公开。

### 3.4 OSS / SLS

- OSS：ali-oss SDK 标准 `Authorization: OSS {accessKeyId}:{HMAC-SHA1(stringToSign)}`（`main/index.js:2769220`），凭据来自 `/front/aliyunoss/token/v2`，不自行实现签名。
- SLS：凭据来自 `/client/ali/log/config`，走官方 SDK。

---

## 4. 登录态

### 4.1 存储位置

| 数据 | 位置 | 证据 |
|---|---|---|
| **token（自动登录/扫码登录恢复用）** | 本地 **SQLCipher 加密的 SQLite** `: 表 `Config`，行 `name="Token"`, `value=<token>`` | 写 `_r.save("Config",{name:"Token",value:e.token})` main/index.js:3888122；读 `getToken(){return _r.get("Config",{name:"Token"})?.value}` main/index.js:3886722 |
| 登录账号密码（勾选"记住"时） | 同一 SQLite 的 `Config` 表 `name="UserInfo"`，值 = `JSON.stringify({username, password})` —— **明文密码落盘** | 写 main/index.js:3886185；读 `getUserInfo()` main/index.js:3886630 |
| 上次登录者 | `CacheData` 表 `key="last_userinfo"` → `{username,uuid}` | main/index.js:3882686 附近 |
| TGT / ST / token（内存） | `M2.tgt` / `CF.tgt` / `M2.token` / `M2.openId`，进程内字段，**不落盘** | main/index.js:3573380 |

数据库细节（`main/index.js:3527877` `wQ`）：
- 文件：`{saveDir}/setup.cfg`（`saveDir` 默认 `%USERPROFILE%\Up366StudentFiles`，平台后缀见 `Up366StudentFiles{""|Uat|Demo|Test}`，main/index.js:627395）。
- 加密：`PRAGMA key="<LZ><suffix>"`（main/index.js:3529280）、首次建库 `PRAGMA rekey="<LZ><suffix>"`（main/index.js:3529190），`LZ = "{3E2A243F-7B8B-45C8-9E7D-885A891820A7}"`（硬编码，main/index.js:3527022），`suffix` 由调用方传入（默认空）。
- 打开失败会自动备份成 `setup-bk{yyyymmddhhii}.cfg` 后重建。
- 无 keytar / 无 OS 凭据库 / 无 electron-store 存 token（electron-store 仅存 `saveDir`、`platform`、`gray`、`devtool` 等，见 `Xp()` main/index.js:626549）。

### 4.2 登录流程与刷新

1. `login(user,pwd)` → `POST /v2/tickets-v3` → `code==-26` 视为成功，保存 `service.tgt = data.tgt`。
2. `Qy()` 收尾：`POST /client/user/currentuser` → 建 `global.CurrentUser` → `GET /front/pc/configs` 拉远程配置（main/index.js:3882686）。任一步 `result.code!=0` 即抛错、登录失败。
3. 之后**每个请求**都带 `u3t = tgt`。
4. 业务接口返回 **`-21`(没有登录) / `-22`(ST 校验失败)** → `POST /v1/serviceTicket {service:<原URL>}` 换 ST：
   - 若返回 `-24`（生成 ST 成功）→ 原请求追加 `?ticket=<serverTicket>` 重放**一次**；
   - 若 `serviceTicket` 返回 `-24` 失败且 `t=!0` → 用 `token` / `openId` / 账号密码**重新登录一次**，再取 ST（main/index.js:3565443）。
5. 失效判定：`-39` "无效的Token" → 在 token 登录路径 `_r.remove("Config",{name:"Token"})`（main/index.js:3888122）；`-26!=result.code` 时前端提示并回登录页。
6. `logout()`：`clearLoginData()`（清 `tgt/token/openId/authorization`）、`MainWindow.clearCookies()`、`_r.remove("Config",{name:"Token"})`、重置 `CurrentUser`（main/index.js:3888764）。
7. 会话 Cookie：`net.request({session: defaultSession, useSessionCookies:true})`，与 WebView 共用 defaultSession；登录/登出会 `clearCookies()`。

### 4.3 凭据读取面（比落盘更值得关注）

`getUserInfo()` / `getToken()` 不只被主进程内部调用，**同时是 JS Bridge 的 RPC 方法**：

- 注册：`No.registerExtensions("Login", Hi)`（main-window.js:4289190 附近），方法进入 RPC 白名单 `_([...],Hi.prototype,"getUserInfo",1)` / `"getToken"`。
- 分发：`ipcMain.on(this.name,async(e,t)=>{this.parent.event=e; ... e.returnValue=await this.target.apply(this.parent,t)})`（main-window.js:4185321）——**只把 event 存到 `this.parent.event`，不做 sender / origin 校验**。
- 这两个方法体里也**没有**读 `this.event.senderFrame`，而同类的 `sendVerifyCode` / `verifyCodeLogin` 都会读——同文件内具备校验能力却未使用（main-window.js:4144607 起）。

后果：任何能发 Bridge 通道的渲染帧（包括被注入 / 被 XSS 的 WebView 页面）都能 `getUserInfo()` 拿到**明文账号密码**、`getToken()` 拿到自动登录 token，**完全绕过 setup.cfg 的 SQLCipher 保护**。

### 4.4 时间同步（影响 `ut` 与 OSS）

任意 HTTP 响应的 `date` 头会调用 `dM(o)` **锁定全局服务器时间**（只锁一次，`fM` 标志），之后 `AA()` 返回「服务器时间 + hrtime 增量」（main/index.js:370132 / 370241）。`getServerTime()`（`POST /front/current-time` → `{currentTime}`）在 OSS `RequestTimeTooSkewed` 时强制重校（`eV()`）。
注意：`ut` 里用的是 `Date.now()`（本机时间），**不是** `AA()`。

---

## 5. WebSocket / SSE / 长连接

**不存在。** 三份 bundle 全文检索：`wss?://` = 0、`WebSocket` = 0、`EventSource` = 0、`text/event-stream` = 0；渲染层 `extract/res/**` 同样为 0。`require("net")` 命中项全部来自第三方库（log4js TCP appender、socks-proxy-agent、ali-oss），非业务长连接。

替代方案是**轮询**：
- 私有云状态 `/minicloud/status`：`setTimeout(...,1e4)` 递归，10s 一次（main/index.js:3536278）。
- 二维码登录状态：URL 下发给渲染层，由渲染层轮询。
- 消息未读：登录后/按需 `POST /client/msg/list-of-pc`。
- 离线队列补交：本地 SQLite（`TaskStuded`/`TestStuded`/`CommonUpload`）驱动，`startUpload()` 扫表重传。

**进程间通道**：直播组件不是 HTTP 长连接，而是 `up366course://?<base64url(params)>` 作为启动参数拉起子进程；参数里含 `token = md5(uid + 房间密码 + "up366")`、`machineCode`、`up366Token`/`up366Password`/`up366Openid`（按登录方式择一）、`UserSig`、`appId`（main/index.js:3701500 附近）。这是明文凭据外泄面（子进程命令行可被同机进程读取）。

---

## 6. 错误码与重试

### 6.1 统一返回体与错误码表（`main/index.js:373724`，`EM` 常量）

```js
je = class { result; data;                       // result = {code, msg}
  static Success(e,t,r){...}                     // code=0, msg="成功"
  static Error(e,t,r){return new je(e, t??EM[e], r)} }
```

| code | msg | 客户端处置 |
|---|---|---|
| 0 | 成功 | |
| -1 | 缺少参数 | 直接透传 |
| -2 | 参数格式错误 | |
| -3 | 无效数据 | |
| -4 | 无权限 | |
| -15 | 接口校验失败 | |
| -16 | UID错误 | |
| **-21** | **没有登录** | → `/v1/serviceTicket` 取 ST 后重放一次 |
| **-22** | **ST校验失败** | 同上 |
| -23 | 没有TGT | 取 ST 失败 → 重新登录 |
| -24 | 生成ST成功 | 内部信号，不是错误 |
| -25 | 生成ST失败 | 返回给调用方 |
| **-26** | **生成TGT成功** | 登录成功信号（`code===-26` 判成功） |
| -27 | 生成TGT失败 | 登录失败 |
| -28 | 请求异常 | `M2.post/get` catch 分支产出，`result.err={code,message,name}` |
| -29 | 账号已被锁定，可修改密码或明天再试！ | 提示 |
| -30 | 账号已禁用，请联系客服！ | 提示 |
| -31 | 登录失败，当前账号登录设备数过多！ | 提示 |
| **-39** | **无效的Token** | 删除本地 Token，回登录页 |
| -98 | 无效请求 | 离线队列：`failed=-1` 停止重试 |
| -99 | 服务器接口发生异常! | 离线队列同上（`-98/-99` 都置 `failed=-1`） |
| 100 | 找不到型号 | 版本检查 |
| -10000 | 网络错误，请稍后重试! | |
| -999 | 客户端错误! | 本地逻辑错误（如缓存查不到 fileId） |

### 6.2 重试策略

| 层 | 策略 | 证据 |
|---|---|---|
| `rf` 传输层 | `retry` 默认 2（`vC` 构造 `new vC({retry:2})`）；仅在 `response.on("error")` 时 `this.retry++ <= opts.retry` 重发；下载类 `retry:0` | main/index.js:3548367 / 3539318 |
| 重定向 | `maxRedirects:30`，301/302/303 跟随；303 强制改 GET | 同上 |
| 超时 | `connectTimeout` / `timeout` 可选（health-status 用 3.5s） | main-window.js:4333872 |
| 私有云降级 | 下载失败先尝试 `?back=1` / `?local=1` 回源，`statusCode 900/909` 分别等 2s/3s 重试 | main/index.js:3539318 |
| md5 校验 | 下载完成后校验 `md5`，不匹配删除文件并按 `localBackDownload` 回源 | 同上 |
| 业务队列 | `HttpQueue` 表：`code!=0` → `failed+=1`，`failed>=4` 删除；`-98/-99` → `failed=-1` 永久停止 | main/index.js:3572731 `execQueue()` |
| 离线成绩队列 | `TaskStuded/TestStuded/CommonUpload.uploaded` 状态机（`0/1/2/-1/-2`），启动时 `startUpload()` 重放 | main/index.js:3878106 |
| 版本探测 | `hw-rpt-cloud-api` 首次访问前探针，结果缓存 1h | main/index.js:3549068 |

### 6.3 其它统一处理

- `session.webRequest` 全局钩子：`onBeforeRequest` 把命中 `redirect://getServiceTicket` 的 POST 体收集后用 `redirectURL` 转发；`onErrorOccurred` 遇 `net::ERR_CONTENT_DECODING_FAILED` 自动 `clearCache()+reload()`（main/index.js:3538397 附近，类 `Af`）。
- 请求耗时监控：`Af.canMonitorUrl`（main/index.js:3539103）排除 `http://127.0.0.1`、`https://127.0.0.1`，其余记录 id→时间戳，`onCompleted/onErrorOccurred/onBeforeRedirect` 计算耗时（钩子注册处 main/index.js:3538397）。
- 代理：`MainWindow.setProxy()` 支持 `{type,ip,port}` → `session.setProxy({proxyRules})`（main-window.js:4333872 附近）。

---

## 7. 安全观察（供后续阶段参考）

1. `Authorization` 头恒空，鉴权完全依赖 `u3t`(TGT) 与 `?ticket=`(ST)；TGT 明文出现在每个请求头。
2. `ut` 不是签名：常量关键词 + md5，**无密钥、无服务端可验的时延约束**，可离线构造；短信发送接口 `/front/user/verify/send` 在登录前匿名调用，ut 是唯一门槛（复现步骤见 §3.2.1，服务端二次风控未验证）。
3. `HMAC-SHA1` 密钥与 SQLCipher 的 `PRAGMA key` 都硬编码在 bundle 内。
4. **明文口令有两条出口**：(a) JS Bridge 的 `Login.getUserInfo()` 无 senderFrame 校验，任意渲染帧可直取（main-window.js:4144607 / 4185321，见 §4.3）；(b) 勾选"记住"时以明文 JSON 落盘 `Config` 表 `name="UserInfo"`（main/index.js:3886185），仅受硬编码 SQLCipher key 保护。网线上传的才是 `md5(md5(pwd)+salt)`（salt `fa&s*l%$k!fq$k!ld@fjlk` 硬编码）。
5. 直播子进程通过命令行参数接收 `up366Password`（明文）与 `up366Token`，同机任意进程可从命令行读取。
6. `local://`/`redirect://` 自定义协议同时开启 `bypassCSP` 与 `CORS`，且渲染进程 `--no-sandbox`（详见 progress.md，属安全组范围）。
