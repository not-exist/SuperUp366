# 错题本（wrongnote-api）H5 端实证

> 目标：`student.up366.cn` 学生端 H5 6.12.1（`chunk-common` 的 `Custom-Agent: student-web-6.12.1`；首页 `Last-Modified: 2026-11-03`），
> 与 PC 学生端 6.11.0 对照。2026-11-05 从线上静态下载全部 JS 入口与懒加载 chunk；未启动 GUI，未使用真实账号。
> 本文补上 `pyapi` README 第 9 节标记为“未验证”的错题本组，并给出一条重要更正：`questions/kill` 是**读**接口。

## 0. 结论先行

1. **wrongnote-api 的鉴权是 TGT → 一次性 ST → `?ticket=`**，不是 `u3t` 请求头。
   无 ticket 实测 `-21`；`?ticket=INVALID` 实测 `-22`；sso 换票无 TGT 实测 `-23`。
2. PC 客户端里的 H5 页面之所以能换票，是因为 Electron 拦截了 H5 发往 `sso.<平台>/v1/serviceTicket` 的 POST
   （`extract/app/dist/preload/index.js@3640984`），改走 `redirect://getServiceTicket/`；
   主进程用 PC 的 TGT 调 `login.getServiceTicket(service,true)`（`main-window.js@4304959`、`main/index.js@3565449`）。
3. 错题本 10 个端点的请求方法、字段、query、响应字段已从 H5 源码取全（§2、§3）。
4. **`/client/wrongnote/student/questions/kill` 是读取**：`wrongnoteKill.html` 页面加载时用它取“待消灭题包”
   （`pathPrefix/paperData/answerData`），不做任何写操作；真正提交消灭结果的是 `/student/question/submit`。
5. `wrong_books` 的「全部」/「练习」哨兵项是 **H5 页面 UI 注入**（`chunk-6ff8c6e6.js@12504`），不是服务端返回。
6. H5 的分页接口把 `pager.currentPage`、`pager.pageSize` **同时**放在 query 与 form body 里；
   `?pager.currentPage=1&pager.pageSize=100`（`chunk-6ff8c6e6.js@15954` 等）。实测 body-only、query-only、两者同时都能正确分页。
7. 2026-11-05 已用有效学生账号完成**只读** e2e：subject/list、stat、book/list、knowledge、course/list、rank、question/show、kill/question/show、questions/kill、finished_tasks、answer_detail v1/v2 全部 `code=0`。写接口只做 dry-run，未真实提交。

## 1. 鉴权链（H5 → PC 客户端）

### 1.1 H5 axios 封装

`student.up366.cn/js/chunk-vendors.bef2299b.js@132057`（模块 `4123`）：

```js
withCredentials: true,
headers: { "X-Requested-With": "XMLHttpRequest",
           "Content-Type": "application/x-www-form-urlencoded; charset=utf-8" },
transformRequest: [e => qs.stringify(e)]
// result.code === -21 | -22:
//   POST <data.serverTicketUrl>  { service: <failed url> }
//   把 ?ticket=<data.serverTicket> 拼回原 URL 重放
// result.code === -23 | -25: 未登录；-4: 无权限
```

- `serverTicketUrl` 实测值为 `http://sso.up366.cn/v1/serviceTicket`；H5 在发请求前 `replace("http","https")`。
- 只有 `-21/-22` 会换票；`-23/-25` 直接按未登录处理。
- 所有接口都是 `POST` + `application/x-www-form-urlencoded`；错误码放在 JSON 的 `result.code`。

### 1.2 PC webview 的换票桥

PC 客户端打开 `student.up366.cn` 页面时：

- `preload/index.js@3640803` 注册 `webRequest.onBeforeRequest`：命中
  `https?://sso.<平台>/…` 且不是 GET/other 的请求，读取 body 并重定向到
  `redirect://getServiceTicket/?<body>`（排除 weChat/qq）。
- `main-window.js@4304959` 注册 `redirect` 协议：取 query 里的 `service`，调用
  `login.getServiceTicket(service, true)`，把结果 JSON 作为响应体返回。
- `main/index.js@3565449` 的 `getServiceTicket`：POST
  `https://sso.up366.cn/v1/serviceTicket`，form `{service}`；期望 `code === -24`，
  取 `data.serverTicket`；`-24` 之外会尝试 token/openId/账号口令重登后再换一次。

结论：H5 页面在 PC 客户端内不需要浏览器 SSO Cookie，换票走 PC 的 TGT；在普通浏览器里则走 SSO Cookie。
两条路的最终产物都是同一个 `?ticket=<ST>`。

### 1.3 未认证实测（2026-11-05）

```bash
# 无 ticket
curl -sS -X POST 'https://wrongnote-api.up366.cn/client/wrongnote/student/subject/list' \
  -H 'X-Requested-With: PC' -H 'Content-Type: application/x-www-form-urlencoded' --data ''
# {"data":{"serverTicketUrl":"http://sso.up366.cn/v1/serviceTicket"},
#  "result":{"code":-21,"msg":"没有登陆"}}

# 无效 ticket
curl -sS -X POST '…/subject/list?ticket=INVALID' …
# {"data":{"serverTicketUrl":"…"},"result":{"code":-22,"msg":"票据超时或者没有票据"}}

# sso 换票（无 TGT）
curl -sS -X POST 'https://sso.up366.cn/v1/serviceTicket' \
  -H 'Content-Type: application/x-www-form-urlencoded' --data 'service=https%3A%2F%2Fwrongnote-api.up366.cn%2F…'
# {"result":{"code":-23,"msg":"找不到tgt"}}
```

- 不带 `X-Requested-With` 时网关返回 `302 → http://sso.up366.cn/login?service=…`；
  带 `PC` 或 `XMLHttpRequest` 都返回 JSON。`pyapi` 发的 `x-requested-with: PC` 可用。
- 只要带 `?ticket=`（即使无效），错误码从 `-21` 变成 `-22`，证明该 host 读取的是 **query 里的 ticket**。
- `u3t` 头在 H5 中完全没出现。

**有效账号鉴权矩阵**（2026-11-05，每次新 ST，同一 URL）：

| 凭证 | `result.code` |
|---|---|
| 仅 `?ticket=<ST>`（无 Cookie、无 u3t） | 0 |
| 仅 Cookie（无 ticket、无 u3t） | 0 |
| 仅 `u3t`（无 ticket、无 Cookie） | -21 |
| ticket + u3t / ticket + Cookie / 三者 | 0 |
| 无任何凭证 | -21 |

结论：**Cookie 与 `?ticket=` 各自都足以鉴权，`u3t` 单独无效**。pyapi 登录后持有 Cookie，并保留 `-21/-22 → ST → ?ticket=` 重试，两条路都可用。

## 2. 端点全表（H5 6.12.1）

H5 源码中的模块 `61f5`（每个入口 bundle 都内联一份，函数体一致）：
`f=subject/list`、`g=subject/stat`、`a=book/list`、`j=knowledge/list/v3`、
`b=course/list`、`e=rank/list`、`h=kill/question/show`、`i=student/question/show`、
`c=questions/kill`、`d=question/submit`。

| # | 方法 | path | body（form） | query | 读/写 |
|---|---|---|---|---|---|
| 1 | POST | `/client/wrongnote/student/subject/list` | 无（空 body） | — | 读 |
| 2 | POST | `/client/wrongnote/student/subject/stat` | `bookId,stageId,subjectId,startTime,endTime` | — | 读 |
| 3 | POST | `/client/wrongnote/student/book/list` | `stageId,subjectId` | — | 读 |
| 4 | POST | `/client/wrongnote/student/knowledge/list/v3` | `bookId,stageId,subjectId,startTime,endTime,orderType,orderField,pager{pageSize,currentPage}` | `pager.currentPage,pager.pageSize` | 读 |
| 5 | POST | `/client/wrongnote/student/course/list` | `stageId,subjectId` | — | 读 |
| 6 | POST | `/client/wrongnote/v2/rank/list` | `courseId,orderType,orderField,pager{pageSize,currentPage}` | `pager.currentPage,pager.pageSize` | 读 |
| 7 | POST | `/client/wrongnote/kill/question/show` | `bookId,uid,stageId,subjectId,killFlag,pager{pageSize,currentPage}[,startTime,endTime]` | `pager.currentPage,pager.pageSize` | 读 |
| 8 | POST | `/client/wrongnote/student/question/show` | `stageId,subjectId,bookId,qtypeId,knowledgeId,pager{pageSize,currentPage},startTime,endTime` | `pager.currentPage,pager.pageSize` | 读 |
| 9 | POST | `/client/wrongnote/student/questions/kill` | `bookId,stageId,subjectId,startTime,endTime` | — | **读**（取待消灭题包） |
| 10 | POST | `/client/wrongnote/student/question/submit` | 见 §3.3 | — | **写** |

时间格式（H5 拼法）：`startTime = "YYYY-MM-DD 00:00:00"`，`endTime = "YYYY-MM-DD 23:59:59"`。
默认时间范围是 `2010-01-01` 到当天；`startTime/endTime` 为空串时按空串发送（不是省略字段）。

### 2.1 响应字段

| 接口 | 响应 |
|---|---|
| subject/list | `data` 为数组；项含 `id,subjectName,subjectId,stageId,bookId,wrongCount,killedCount,killedCountLately` |
| subject/stat | `data` 为对象：`wrongCount,killedCount,killedCountLately` |
| book/list | `data` 为数组；项含 `bookId,bookName` |
| knowledge/list/v3 | `data.{list,pager}`；行含 `qTypeId,qTypeName,knowledgeId,knowledgeName,wrongTimes,wrongCount` |
| course/list | `data` 为数组；项含 `courseId,courseName` |
| rank/list | `data.{list,pager}`；行含 `realname,wrongCount,killedCount,recentKilledCount`；`pager` 另有 `offset` |
| kill/question/show、student/question/show、questions/kill | `data` 为题包对象；见 §3.1 |

## 3. 题包与提交

### 3.1 题包响应（kill/question/show、student/question/show、questions/kill）

`wrongnoteView.916c932e.js@14856` 的 `initPaper` 与 `wrongnoteKill.fcef1833.js@9676`：

```
data = {
  pathPrefix,          // 资源前缀，拼 paperData/answerData 里的相对地址
  paperData,           // 试卷 XML
  answerData,          // 标准答案 XML
  userAnswerData,      // 学生作答 XML（show 接口有；questions/kill 页面未用）
  list: [{questionId, ...}],
  pager: {currentPage, pageSize, pageCount, offset?}
}
```

- `data` 为 `{}`（或 `kill/question/show` 的今日列表返回 `[]`）表示**没有题**，不是响应损坏。
  `wrongnoteView.916c932e.js` 用 `isEmptyObject(data)` 判断后显示 `hasNoQuestion`；
  `wrongnoteKill.fcef1833.js@13625` 判断为空后弹“恭喜你，所有错题已消灭”。实测 `killFlag=1` 返回 `data:[]`，`killFlag=0` 返回完整题包。
- 分页大小：show 接口 10；knowledge/rank 100（`3ebc` 模块 `PAGE_SIZE=100`）。

### 3.2 `questions/kill`（读）

调用点 `wrongnoteKill.fcef1833.js@9234`：

```js
extractWrongQuestionList: function () {
  var e = {bookId: q, stageId: I, subjectId: A, startTime: D, endTime: U};
  return Object(f["c"])(e).then(function (e) { … return e.data || {}; });
}
```

- 没有 `uid`、`pager`、`killFlag`。
- 返回题包（§3.1），页面随后 `initPaper(data)` 渲染待消灭题。
- **不是写接口**：没有 dry-run、没有提交语义；把它当写接口会误伤。

### 3.3 `question/submit`（写，消灭错题提交）

调用点 `wrongnoteKill.fcef1833.js@11384`、`wrongnoteView.916c932e.js@16944`；
参数构造在 `wrongnoteView.916c932e.js@16675`：

```js
getSubmitParams: function (submitData) {
  var result = {
    answerdata: paperInfo.userAnswerXML,                 // xotPaperText.getSubmitData().userAnswer
    mark_datao: "<elements>" + submitData.markData + "</elements>"
  };
  var seconds = Math.ceil((endTime - startTime) / 1000);
  var ids = (submitData.rightQstIds || []).join(",");
  return {
    uid: String(uid),
    result: JSON.stringify(result),
    startTime: String(startTime),   // 毫秒时间戳（服务器时间）
    endTime: String(endTime),
    seconds: seconds,
    rightQstIds: ids                // 逗号分隔，无则 ""
  };
}
```

- `startTime` 在页面 `created` 时取 `/front/current-time` 类接口的 `currentTime`（毫秒）；
  `endTime` 在提交前重新取一次服务器时间。
- `mark_datao` 必须是 `<elements>…</elements>` 包裹的 XML；`answerdata` 是 xot 引擎产出的作答 XML。
- H5 只检查 `result.code === 0`，随后本地渲染报告，不读 `data` 字段。

## 4. H5 调用点索引

| 页面/chunk | 调用 | 证据 |
|---|---|---|
| `chunk-61a2f2be.js` 错题本首页 | `subject/list` 无参 | `@13793` |
| `chunk-6ff8c6e6.js` 错题本主页面 | `book/list` `{stageId,subjectId}`；`subject/stat`；`knowledge/list/v3` | `@12381`、`@23459`、`@16502` |
| `chunk-6ff8c6e6.js` 书列表 | UI 注入 `{bookId:"0",bookName:"全部"}` / `{bookId:"WORK",bookName:"练习"}` | `@12504` |
| `chunk-9ee04f5e.js` 排行页 | `course/list`；`rank/list` | `@3387`、`@4063` |
| `wrongnoteView.916c932e.js` 已消灭/剩余列表 | `kill/question/show`；`student/question/show`；`question/submit` | `@13877`、`@14530`、`@16944` |
| `wrongnoteKill.fcef1833.js` 消灭错题页 | `questions/kill`；`question/submit` | `@9345`、`@11384` |

## 5. 对 `pyapi` 的直接修正项

| `pyapi` 现状 | 实证 | 建议 |
|---|---|---|
| `kill_wrong_questions(payload, dry_run, confirm)` 当写接口 | `questions/kill` 是读，参数 `{bookId,stageId,subjectId,startTime,endTime}` | 改为读方法，返回题包；删除 dry-run 语义 |
| `submit_wrong_question(payload)` 透传 | 写接口，payload 字段固定（§3.3） | 保留；文档写明 6 个字段 |
| `wrong_books` 在 API 层注入「全部/练习」 | 哨兵由 H5 页面注入 | 库返回原始数组，或明确标注是库的便利行为 |
| `wrong_knowledge` / `wrong_rank` 只把 pager 放 body | H5 同时放 query | 请求时追加 `?pager.currentPage=&pager.pageSize=` |
| `wrong_killed_questions(**params)` / `wrong_question_show(**params)` 透传 | 参数固定（§2 表 #7/#8），且 pager 在 query | 改为显式关键字参数，构造 query |
| `WrongRank.degree` | 排行行没有 `degree`；实际是 `realname,wrongCount,killedCount,recentKilledCount` | 删除 `degree`，补齐字段 |
| 分页/题包接口遇 `data={}` 抛 `Up366ResponseError` | `{}` 表示没有题 | 返回空页，不抛 |
| `start_time=None` 经 `form_encode` 变成 `"None"` | H5 总是发 `YYYY-MM-DD 00:00:00` 或空串 | 省略 None 或补默认时间 |
| `wrong_rank`/`wrong_knowledge` 的 `orderField` 默认 | 排行可 `wrongCount/killedCount/recentKilledCount`；知识点可 `qTypeName/knowledgeName/wrongTimes/wrongCount` | 文档列出枚举 |

**只读 e2e 实测（2026-11-05）**：

- `wrong_knowledge(book_id="0")` 返回 `data.{list,pager}`；`wrong_question_show` 返回 `data.{pathPrefix,paperData,answerData,userAnswerData,list,pager}`，`option` 里的 `pager` 含 `totalCount/indexPageCount/offset`。
- `wrong_killed_questions(kill_flag=1)` 返回空 `data:[]`；`kill_flag=0` 返回题包和 `list`。
- `wrong_kill_paper(book_id="0")` 返回 `{pathPrefix,paperData,answerData}`；没有待消灭题的书返回 `{}`。
- `wrong_rank` 行字段为 `realname,wrongCount,killedCount,recentKilledCount,uid`；`pager` 含 `offset/totalCount/indexPageCount`。
- `finished_tasks` 的 GET 与 POST 返回**完全相同**的 `taskId` 集合与字段值；额外 `courseId` 被忽略。行字段：`addDate,batchId,bookId,chapterId,firstScore,maxScore,percent,score,seconds,studyDate,taskId,uid`。
- `answer_detail` v1（GET）与 v2（POST）都要求 `courseId,uid,bookId,taskId`；`taskId` 取 `contentId`（传 `jobId` 会得到空 `paperInfo`）。v2 缺 `courseId` 返回 `code=-1,msg=param_missing`。v1 返回 `data.{paperInfo,userAnswer}`；v2 额外返回 `batchId,courseName,percent,seconds,studyDate,submitDate,taskName`。

## 6. 复现

```bash
# H5 入口与 runtime
curl -sS https://student.up366.cn/ -o index.html
curl -sS https://student.up366.cn/wrongnoteView.html -o wrongnoteView.html
curl -sS https://student.up366.cn/wrongnoteKill.html -o wrongnoteKill.html
# 按 runtime 的 chunk 映射批量下载 js/*.js（见 work/h5/chunks.txt 的生成方式）

# 未认证探针（不触达账号）
curl -sS -X POST 'https://wrongnote-api.up366.cn/client/wrongnote/student/subject/list' \
  -H 'X-Requested-With: PC' -H 'Content-Type: application/x-www-form-urlencoded' --data ''
curl -sS -X POST 'https://wrongnote-api.up366.cn/client/wrongnote/student/subject/list?ticket=INVALID' \
  -H 'X-Requested-With: PC' -H 'Content-Type: application/x-www-form-urlencoded' --data ''

# PC 侧鉴权桥
python tools/grep_ctx.py extract/app/dist/preload/index.js "redirect://getServiceTicket" 1500 900 1
python tools/grep_ctx.py extract/app/dist/main/main-window.js "registerStringProtocol(\"redirect\"" 1500 900 1
```

## 7. 实测状态

- 2026-11-05 已用有效学生账号完成只读 e2e；上述 10 个读取路径全部 `code=0`，返回结构与本文章节一致。
- 写接口 `question/submit` 只做 dry-run，**未真实提交**（会改动账号数据）；payload 六字段由 H5 源码确认。
- 鉴权矩阵已确认：Cookie 或 `?ticket=` 任一即可，`u3t` 单独无效。
- 分页已确认：body-only / query-only / 两者同时三者等价；H5 同时发，库也同时发。
