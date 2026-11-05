# 逐句跟读：经验总结与操作手册

> 本文是 2026-11-03 真实提交（Unit 4 课文1 逐句跟读）之后沉淀的**可复现流程**，
> 以及当日踩过的所有坑。事实均以客户端 bundle / 真实报文 / 实测返回为准，不含推测
> （推测会明确标注）。
>
> 配套文档：`recon/readaloud-complete.md`（协议全貌）、
> `recon/readaloud-submit-verify.md`（本次提交的完整记录）。

---

## 0. TL;DR

```
启动并登录客户端
  → 查 job/info 拿齐 id（studyTaskId = jobId！）
  → 3 段 mp3 上传到 OSS（objectName 本地算）
  → 构造 TaskStuded 行（result 里塞 answerdata/markdatao XML）
  → 用 AppInfo.httpRequest 直发 /client/task/score/submit/v2
  → 补本地簿记三表 → 重启客户端
```

三个致命坑，踩任何一个都会"提交成功但显示未完成"：

| 坑 | 后果 |
|---|---|
| `studyTaskId` 自己编了一个 | 服务端 `finished-task` 有记录，但 `job.taskStatus` 恒为 0 → **所有**客户端显示未完成 |
| 分数/XML 自己编，音频不真实 | 能过，但服务端可能异步改分（见 §4.1） |
| 提交后不写本地 `taskscore` | 课本页（走本地映射的那条路径）可能不刷新 |

---

## 1. 已核实的事实

| # | 事实 | 证据 |
|---|---|---|
| F1 | 完成状态由**服务端 job** 决定：`job/info` 的 `taskStatus`（0 未交 / 1 已交）、`submitTime`、`stuStatus`（2 = 已提交） | 本次提交前后对比 |
| F2 | **`studyTaskId` 必须等于作业列表里的 `jobId`** —— 它不是随机 uuid | 样本2 `jobId=4578C972…` 与其 `studyTaskId` 完全一致；本次用 `jobId=251C21…` 提交后 `taskStatus` 才 0→1 |
| F3 | `taskId = chapterId = pageId = contentId`（同一个 32 hex）；`paperId` 独立 | `job/info` 返回 |
| F4 | 分支 A 接口 `POST /client/task/score/submit/v2`，`application/x-www-form-urlencoded`，三个字段：`tasksJson`(JSON 字符串，内含 `[row]`)、`ut`、`submitType` | 客户端 `submitTaskScore` |
| F5 | `ut = r[:10] + md5(md5(tasksJson + "submitTaskToken-pc-987654" + r)) + r[10:]`，`r = md5(String(Date.now()))`，长度 64 | 用本地 2 条真实报文逐字节验证通过（`tools/test_readaloud.py`） |
| F6 | 传输层直接用客户端 `AppInfo.httpRequest(method, url, body)` —— 自动带 7 个公共头、TGT、`-21/-22` 换 ST 重放 | 实测，免去自己登录取 TGT |
| F7 | `objectName` **本地生成**，不等上传回填：`storage/{userType}/study/a/{yyyy/mm/dd}/{uid}/{file}`（mp3→`a`） | `buildOSSObjectName`；`tools/test_objectname.py` 对拍 |
| F8 | 上传用 `AppInfo.uploadFile(localPath, objectName)`，第 2 参非空则**原样使用** | preload `uploadFile` |
| F9 | 服务端唯一看得到的音频引用 = `result.testInfo.answerdata` 里的 `<answer_url>`（题级一个，句级复用） | 真实报文 |
| F10 | **录音粒度是题不是句**：15 句 → 3 段音频，一题一段 | 引擎一次会话按 `.net` 切句 |
| F11 | `qstJson`：提交前拍平成数组，每题 1 条 `elementType=1` + N 条 `elementType=2`（`elementId-N`） | 真实报文 280 样本 |
| F12 | `markdatao` 的 `<mark_data>` 每题一条，`<score>` 与 `answerdata` 的 `<answer_score>` 对应 | 客户端 `getResultByEngineScore`：`result` 1 满分 / 2 零分 / 3 部分分 |
| F13 | 本地 SQLCipher：key = 常量 `{3E2A243F-…}` **+ 用户名**；per-user 库在 `C:/Up366StudentFiles/user/<账号>/setup.cfg`；**表名全小写** | `openDataBase` |
| F14 | 课本页"完成"判据 `finished = taskScore ? 1 : 0`，`taskScore` 来自「本地 `taskscore` 表」+「服务端 `finished-task` 列表」合并（本地优先） | `toViewBookTask` / `initChapter` |

---

## 2. 操作流程（本次实际执行顺序）

### Step 0 —— 启动并登录客户端

```powershell
& 'H:\project\up366\harness\dev-launch.ps1' -Offscreen -Keep -WaitSec 16
& 'H:\project\up366\harness\offscreen.ps1' -X 80 -Y 60   # 窗口必须在屏幕内，否则 document.hidden
```

> 窗口若在屏幕外，Electron 判 `document.visibilityState = hidden`，
> 页面内 `click()` 与路由跳转全部失效（`eval` 仍可用）。

### Step 1 —— 取作业 id（关键：`studyTaskId = jobId`）

```bash
export MSYS_NO_PATH_CONV=1
U3_FRAME="=local://res/index.html#/" node harness/u3.js eval "(async()=>{
  const r=await window.AppInfo.httpRequest('post',
    'https://studytask-api.up366.cn/client/student/job/info',
    {courseId:9222783, jobId:'<jobId>', contentId:'<taskId>', bookId:'<bookId>'});
  return JSON.stringify(r.data)
})()"
```

拿齐：`contentId/taskId/chapterId/pageId`、`paperId`、`bookId`、`courseId`、
`extParam.taskScore`（满分）、**`jobId` → 就是 `studyTaskId`**。

### Step 2 —— 准备音频

- 一题一段，顺序与 `answer.json` 的 `sentences[]` 一致。
- 参考时长在 `paper.xml` 的 `extend2` 里（本次 82/86/135s）；实际音频长度不必相等，
  但明显超长会让机器评分变差（见 §4.1）。

### Step 3 —— 上传 + 构造行

```bash
python tools/readaloud_submit.py \
  --mp3 "q1=test000/1.mp3,q2=test000/2.mp3,q3=test000/3.mp3" \
  --scores 8,8,8          # 先 dry-run 看映射与 percent
```

产物：`work/readaloud-prep/row.json`（含 `result` XML、`qstJson` 18 条、`ut` 所需字段）。
脚本内部：

1. `AppInfo.uploadFile(local, objectName)` 上传 3 个 mp3
2. `objectName = storage/{utype}/study/a/{yyyy/mm/dd}/{uid}/ra{q}.mp3`
3. 回填 `<answer_url>`

### Step 4 —— 提交

```bash
python tools/readaloud_post.py --row work/readaloud-prep/row.json --send
```

成功返回 `{"result":{"code":0,"msg":"success"}}`。

### Step 5 —— 核对（三处，缺一不可）

```bash
# ① 服务端成绩记录
POST https://dsb-api.up366.cn/client/book/finished-task/list {bookId, courseId}
#    按 taskId 查 score / firstScore / seconds / batchId

# ② 作业提交状态（这才是"完成"的判据）
POST https://studytask-api.up366.cn/client/student/job/info {…}
#    期望 taskStatus:1, submitTime:<非0>, stuStatus:2

# ③ 本地表
taskstuded.uploaded = 2（否则客户端重放撞 -13）
```

### Step 6 —— 补本地簿记（课本页显示需要）

| 表 | 写入 |
|---|---|
| `taskscore` | `bookId, taskId, type=2, pid=taskId, study=2, score, weight=0, percent=100, studyDate` |
| `studytaskrecord` | `taskId, studyTaskId, score, studyDate` |
| `taskstuded` | `uploaded=2, msg=已提交` |

### Step 7 —— 重启客户端验证

`initChapter` 只在加载课本时跑，**刷新页面无效，必须重启或重进课本**。

---

## 3. 踩坑清单

| # | 现象 | 根因 | 对策 |
|---|---|---|---|
| 1 | 提交返回 `code:0`，但所有客户端都显示未完成 | `studyTaskId` 自造，服务端无法关联到 job | **`studyTaskId = jobId`** |
| 2 | `NetBrowser.submitTask({...})` 抛 SQLite 绑定错误 | 该方法签名是 **10 个位置参数**（`bookId, chapterId, taskId, taskNo, score, percent, seconds, result, studyTaskId, courseId`），且**不接受 qstJson** | 别用它，直发 `/submit/v2` |
| 3 | 页面里调 `jsBridgeObj.submitTaskV1` 无效 | 依赖 `MainWindow.views.current` 上下文，裸调没有 | 同上 |
| 4 | 插了 `taskstuded` 行，客户端上传器不捡 | 客户端与学生库是**独立 SQLite 连接**，看不到外部进程的写入 | 直发 POST，别指望队列 |
| 5 | `manualUploadTask()` 调了没反应 | 同上 | 同上 |
| 6 | 页面 `click()` 无效、路由不跳 | 窗口在屏幕外 → `document.hidden` | `offscreen.ps1 -X 80 -Y 60` |
| 7 | `read_setupcfg.js` 以文件方式运行无输出 | 厂商 fs 补丁破坏 `.js` 路径解析 | 用 `-e "$(cat tools/read_setupcfg.js)"` 内联运行（argv 起点不同，脚本已兼容） |
| 8 | 查 `TaskStuded` 报"表不存在" | 表名**全小写**，且 per-user 库的 key 要**拼用户名** | `select * from taskstuded`；`PRAGMA key = '{…}'+username` |
| 9 | 提交后客户端重放，撞 `-13 重复作答` | `taskstuded.uploaded` 仍为 0 | 提交成功即置 2 |
| 10 | 服务端 `percent` 不是我传的值 | 服务端按 `score/maxScore` 自算 | 别指望 percent |

---

## 4. 未解问题与风险

### 4.1 分数会被服务端改（待定位）

本次上报 `score=24`（8×3），提交后首次读到 `score=24 / maxScore=24`；
稍后再读变成 **`score=8.2 / maxScore=8.2 / firstScore=8.2 / percent=100`**，
`batchId`、`studyDate`、`addDate` 都没变 —— 是**同一条记录被就地改写**。

两种可能（都未验证）：

1. **服务端对口语题做异步机器复评**，用 ASES 重算音频得分（8.2 像是机器分），
   并把 `score` 与 `maxScore` 都写成该值。
2. 客户端在某次加载时回写了成绩（batchId 未变，可能性较低）。

> 影响：**"自定义分数"这条路不可靠**。若目标是拿高分，正确做法是让音频本身读好，
> 而不是填分数。判定方法：看 `firstScore` 是否等于你上报的值。

### 4.2 其它

- **一次性判重**：服务端按 `(uid, studyTaskId)` 判重，重复提交返回 `-13`。
  同一 job 只有一次机会（本次因 firstScore 被改，已无法再验证）。
- **音频公开可访问**：`<answer_url>` 是 OSS 直链（`HTTP 200`），老师侧可见。
- **残留文件**：早期探测上传的 `storage/102/study/a/2026/11/03/39370562/probe_test000_1.mp3`
  仍在 OSS 上（7.9MB），客户端无删除接口。
- **账号/口令不写进仓库**：`readaloud_job.py` 走账密时需即时传入；更稳的是走已登录客户端的
  `AppInfo.httpRequest`，完全不碰 TGT。

---

## 5. 工具索引

| 文件 | 作用 |
|---|---|
| `tools/readaloud_prepare.py` | 从本地包导出 3 题结构（elementId/句子/分值/时长）→ `task.json` |
| `tools/readaloud_submit.py` | 上传 3 段音频 + 构造 `row.json`（dry-run 默认） |
| `tools/readaloud_post.py` | 直发 `/client/task/score/submit/v2`（本地算 ut） |
| `tools/readaloud_build.py` | 报文构造器（分支 A / C 都支持，离线用） |
| `tools/readaloud_upload.py` | 纯 Python 的 OSS 上传（不依赖客户端运行） |
| `tools/readaloud_job.py` | 用账密定位作业 id |
| `tools/read_setupcfg.js` | 本地库读写（dump / userinfo / insert-records / insert-score / backup） |
| `tools/test_readaloud_all.py` | 核心语义测试：ut 对真实报文逐字节 + objectName 对客户端公式 |
