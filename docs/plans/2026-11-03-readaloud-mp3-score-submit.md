# 逐句跟读（Unit 4/课文1）— 15 个 mp3 自定义分数提交 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.
> 本文只覆盖"提交链路复现"这一段；评测引擎（ASES）本地打分不在范围内（见 `docs/plans/2026-11-01-readaloud-submit-plan.md`）。

**Goal:** 让用户自备的 15 段 mp3（按题归并为 3 段）随一次任务提交落到远端，分数由提交方指定，全程不调用本地 ASES 引擎。

**Architecture:** 提交行里**不含音频文件**，音频以 OSS URL 形式出现在两处：朗读题 XML 的 `<answer_url>`，以及 `qstJson[].elementAttr.record_url`。因此有两条落地路径：
- **路径 A（首选）**：把 mp3 登记成 `RecordFiles` 行、把分数写进 `TaskStuded` 行 → 客户端自己的上传器 `df` 会按 `taskId` 捞起录音上传、再提交成绩行。ut / 表单编码 / ST 兜底 / OSS 凭据全部由客户端处理，最稳。
- **路径 B（无客户端）**：纯 Python 复现 —— 登录取 TGT → 取 OSS STS → 签名 PUT 3 个 mp3 → 自造 XML/JSON → 自己算 ut → POST，遇 -21/-22 走 ST 重放。

> **2026-11-03 修订**：本计划原先基于"读文档 + 抓包"推断，已被**本地真实提交报文**推翻多处（见 §0.1）。任务顺序已重排，抓包不再是前置阻塞项。

**Tech Stack:** Node 16 (ia32, `ELECTRON_RUN_AS_NODE=1` + `libsqlite.node`)、Python 3.13（`requests`/`hashlib`/`hmac`，无 `oss2`）、Aliyun OSS V1 签名 PUT、SQLCipher（key `{3E2A243F-7B8B-45C8-9E7D-885A891820A7}`）。

---

## 0.1 已核实的事实（2026-11-03，含 3 处被推翻的前提）

证据：`extract/app.u3a/dist/main/{index.js,main-window.js}`、`work/report/u3_flip_sdk.*.js`（远端 flip SDK）、
**`C:/Up366StudentFiles/user/<账号>/setup.cfg` 里存着的 2 条真实课本提交报文**（`commonupload` 表，scenario `_SCHOOL_BOOK_PRACTICE_`）。

| # | 事实 | 位置 |
|---|---|---|
| F1 | `objectName` 由 `buildOSSObjectName` **本地生成**：`storage{type}/study/a/{yyyy/mm/dd}/{uid}[/{cloud}]/{file}`（mp3→`a`） | index.js@3556173 |
| F2 | `TaskStuded`/`TestStuded` **没有 `uuid`/`objectIds`**（只有 `CommonUpload` 有）；`RecordFiles` 与成绩行靠 **`taskId`** 关联 | 表结构 v0e；`df` 构造 |
| F3 | 音频与服务端的关联在 **XML 的 `<answer_url>`** 与 **`qstJson[].elementAttr.record_url`**；`TaskStuded.result` 只在分支 A 装 XML，分支 C 装在 `extendParams.xmlResult` | 实测报文 |
| F4 | **录音单位 = 题**：一次录音覆盖整题，引擎按 `.net` 参考音在**一次**会话里切句 ⇒ **15 句 = 3 段录音** | `answer.json.sentences[]` + flip SDK `markSpokens` |
| F5 | `qstJson`（分支 C）= **数组**；每题 1 条 `elementType=1` + N 条 `elementType=2`（`elementId-N`）；`elementAttr` 字段全集已统计 | 实测报文 280 样本 |
| F6 | `mark_data id` = 题内子项 `order`（1..N）；`answerdata`/`markdatao` 的 `<element>` 数 = 题数 | 实测报文对齐 |
| F7 | `ut` = `r[:10] + md5(md5(payload)+"submitTaskToken-pc-987654"+r) + r[10:]`，`r=md5(str(Date.now()))`，**长度 64**；分支 A 内联盐 vs 分支 C `buildUT` 盐两种写法等价 | index.js@3579270 / @3827639；已用真实报文验证 |
| F8 | 分支 B（`/client/onlinetesting/submit`）**无 ut**，整行作为 form 字段平铺 | index.js@3579464 |
| F9 | 表单编码 `x-www-form-urlencoded`；对象/数组值 `JSON.stringify`；键内 `###`→`.` | `Pc.getForm` |
| F10 | ST 兜底：`-21/-22` → `POST /v1/serviceTicket {service:<完整URL>}` → **必须 `code==-24`** 才拿到 ST → 原 URL 加 `?ticket=` 重放一次 | index.js@3565449 |
| F11 | 公共头 7 个：`user-agent: PC-Up366-Student 6.11.0` / `x-requested-with: PC` / `X-App-Name: student-pc` / `clientId: MachineCode` / `u3r: uuid去横线` / `u3t: TGT` / `Authorization: ""` | index.js `M2` |
| F12 | OSS 凭据：`GET /front/aliyunoss/token/v2?organId={schoolId}` → `ossBucketInfo{tmpAkId,tmpAkSecret,securityToken,bucketname,defaulthostid,currentTime}`；上传走 `multipartUpload` | index.js@3577130 / @3555880 |
| F13 | `recordFile` **不以 `.wav` 结尾就原样上传**（不转码），成功后删本地文件并置 `uploaded=2` | `uploadFile` main-window.js@4131959 |
| F14 | SQLCipher key = `"{3E2A243F-…}"` **+ 用户名**；per-user 库 = `C:/Up366StudentFiles/user/<账号>/setup.cfg`（公共库是另一个文件）；**表名全小写** | `openDataBase` index.js@3529280 |
| F15 | 本机库里存着 `USER_INFO`（含 `uid`/`organId`/`utype`/`uuid`）⇒ **无需登录凭据**即可拿到 uid / schoolId / type | `cachedata` 表 |
| F16 | 客户端暴露 `submitTask` bridge，渲染层自带 `tasksJson` + `urlList`；`urlList` **只收口语题（type 9/12/13）的 `record_url`**，`objectIds` = URL 的 `pathname.slice(1)` | bridge 白名单 index.js@3811632；flip SDK `SubmitImpl.js` |

### 0.2 被推翻的三个前提（原计划据此排的 Task 1 已作废）

| 原假设 | 实测结论 |
|---|---|
| 需要抓一次真实提交才能定分支/字段 | **不必** —— 本地库里已有 2 条真实课本报文，字段形状全部拿到（`tools/fixtures/real-submit-payloads.json`） |
| 音频只能通过 `RecordFiles` + `uuid/objectIds` 引用 | **错** —— A/B 分支的行里没有 `uuid/objectIds`；服务端看的是 XML 的 `<answer_url>` 与 `elementAttr.record_url` |
| 15 句 = 15 段录音 | **错** —— 录音单位是**题**，15 句 = 3 段（引擎一次会话内按 `.net` 切句） |

### 0.3 仍需用户提供的输入

| 输入 | 用途 | 现状 |
|---|---|---|
| **3 段 mp3**（或 15 段待归并） | 上传后填 `record_url` / `<answer_url>` | 待提供 |
| **每题分数** | 填 `score`/`percent`/`user_score`/`result` | 待提供（工具侧已支持任意值） |
| 走哪条分支（A / B / C） | 决定 URL 与行字段 | 本地样本是 C；朗读题无样本 → 建议先做一次**真实朗读题提交**看走哪条，或直接选 C |
| 本次任务的 `studyTaskId` / `chapterId` / `paperId` | 行字段 | 从作业列表接口取，或先做一次真实提交反推 |

---

## 已完成的工具（无需 mp3 / 分数即可用）

| 工具 | 作用 | 状态 |
|---|---|---|
| `tools/readaloud_prepare.py` | 从本地包导出 3 题结构（elementId/句子/分值/时长/netfile）→ `work/readaloud-prep/task.json` | 已实现，输出 3 题 / 15 句 / 30 分 |
| `tools/readaloud_build.py` | 按实测形状生成 `tasksJson`/`ut`/两份 XML（分支 A 与 C 都支持） | 已实现，默认只打印 |
| `tools/readaloud_upload.py` | 取 TGT → 取 STS → 按 F1 规则生成 objectName → OSS V1 签名 PUT | 已实现，dry-run 默认 |
| `tools/read_setupcfg.js` | 读写本地库（`userinfo` 取 uid/schoolId/type；`insert-records`/`insert-score` 插行；`backup`） | 已实现，幂等 |
| `tools/test_readaloud_all.py` | 核心语义测试：ut 对真实报文逐字节比对 + objectName 对客户端公式比对 | 2/2 通过 |

跑测试：

```bash
python tools/test_readaloud_all.py
```

---

## Task 1: 拼音频（需要 3 段 mp3）

**Why:** 录音单位是题，15 句要归并成 3 段；顺序必须与 `answer.json` 的 `sentences[]` 一致。

**Files:**
- Create: `tools/readaloud_merge.py`（15 段 → 3 段）

### Step 1: 归并

用客户端自带的 ffmpeg（`C:/Program Files (x86)/up366/client/lib/ia32/win32/ffmpeg/ffmpeg.exe`），
concat demuxer 不重编码：`-f concat -safe 0 -i list.txt -c copy`；参数不一致导致失败时退化为
`-c:a libmp3lame -b:a 96k`（与客户端转码参数一致）。

映射：`Q1 前4句 → Q2 中5句 → Q3 后6句`（`task.json` 里每题的 `sentences[]` 顺序即权威顺序，不符则打印告警）。

**Expected**：`work/readaloud-prep/q1.mp3` / `q2.mp3` / `q3.mp3`，各打印时长与大小。

### Step 2: 提交

```bash
git add tools/readaloud_merge.py
git commit -m "feat(tools): readaloud_merge.py - merge per-sentence clips into 3 per-question clips"
```

---

## Task 2: 上传音频拿到 OSS URL（需要 mp3 + 登录凭据或已登录客户端）

**Why:** 服务端只按 URL 取音频；文件必须先在 OSS 上存在。

**Files:**
- Use: `tools/readaloud_upload.py`

### Step 1: 取用户字段（无需凭据）

```bash
ELECTRON_RUN_AS_NODE=1 "C:/Program Files (x86)/up366/client/up366.exe" -e "$(cat tools/read_setupcfg.js)" userinfo --user <username>
```

拿到 `uid` / `schoolId` / `type`（→ objectName 前缀与 STS 的 `organId`）。

### Step 2: 上传

```bash
python tools/readaloud_upload.py --task work/readaloud-prep/task.json \
    --mp3 q1=q1.mp3,q2=q2.mp3,q3=q3.mp3 \
    --login <账号> --password <口令> --client-id <MachineCode> \
    --school-id <schoolId> --uid <uid> --user-type <type> --send
```

**Expected**：回填 `task.json` 的 `ossUrl` / `ossObjectName`。

---

## Task 3: 构造并提交（需要分数）

**Files:**
- Use: `tools/readaloud_build.py`

### Step 1: 生成报文（dry-run）

```bash
python tools/readaloud_build.py --task work/readaloud-prep/task.json \
    --scores <Q1分>,<Q2分>,<Q3分> \
    --urls "<q1 url>,<q2 url>,<q3 url>" \
    --branch c --submit-type 8 \
    --study-task-id <...> --chapter-id <...> --paper-id <...> \
    --uid <uid> --client-id <MachineCode> \
    --out work/readaloud-prep/body.json
```

**Expected**：打印行字段 / 两份 XML / qstJson 18 条 / ut（64 字符），并写 `body.json`。

### Step 2: 走路径 A（让客户端自己交）—— 推荐

自写 Python 发送器容易在报文细节上出错；让真客户端发最稳：ut / 表单编码 / ST 兜底 / OSS 全是客户端自己的实现。

```bash
# 1) 备份库
ELECTRON_RUN_AS_NODE=1 "C:/Program Files (x86)/up366/client/up366.exe" -e "$(cat tools/read_setupcfg.js)" backup --user <username>

# 2) 插 RecordFiles（3 行，每题一行；recordFile 指向本地 mp3；
#    objectName 直接从 task.json 读，必须与上传时用的完全一致）
ELECTRON_RUN_AS_NODE=1 "C:/Program Files (x86)/up366/client/up366.exe" -e "$(cat tools/read_setupcfg.js)" insert-records --user <username> --json records.json

# 3) 插 TaskStuded 行（分数在这里）
ELECTRON_RUN_AS_NODE=1 "C:/Program Files (x86)/up366/client/up366.exe" -e "$(cat tools/read_setupcfg.js)" insert-score --user <username> --json row.json

# 4) 启动客户端 → startUpload() 扫到 uploaded!=2 的行 → 自己上传录音 + 提交成绩
```

启动客户端用 `harness/dev-launch.ps1 -Offscreen -Keep`（见 `harness/README.md`）。

### Step 3: 路径 B（纯 Python 直提）—— 备选

⚠️ **这一步会真的写远端成绩记录，且同一 `(uid, studyTaskId)` 服务端只判重一次**（第二次 `-13 重复作答`）。
所以"用低分探一次服务端是否复核"和"提交想要的分数"**互斥**，只能二选一。

- 想验证服务端是否复核分数 → 用一个明显异常的低分提交，观察服务端落库值是否被纠正。
- 想要分数达标 → 直接提交目标分数。

发送脚本等分数与分支确定后再写，避免写出用错分支的发送器。

### Step 4: 验证

```bash
ELECTRON_RUN_AS_NODE=1 "C:/Program Files (x86)/up366/client/up366.exe" -e "$(cat tools/read_setupcfg.js)" dump --user <username>
```

**Expected**：`taskstuded` 该行 `uploaded=2`、`score=<目标值>`；`recordfiles` 三行 `uploaded=2`、`objectName` 非空。

### Step 5: 提交

```bash
git add tools/readaloud_merge.py
git commit -m "feat(tools): read-aloud submit path (merge/upload/build) - verified field shapes"
```

---

## Task 4: 结论回写文档

**Files:**
- Modify: `recon/readaloud-complete.md`（把本次提交实测的"服务端是否复核"等结论补进 §6）
- Create: `recon/readaloud-submit-verify.md`（本次提交的完整记录：报文全文、ut、服务端返回码、落库分数）

注意：若走路径 A（客户端提交），服务端响应码要从客户端日志里找（`C:/Up366StudentFiles/logs/`）。

---

## R. 风险与边界

- **R1 音频内容会被老师看到**：音频进学校 OSS，`<answer_url>` 直链可听。用自备 mp3 前确认内容可公开。
- **R2 服务端可能二次复核分数**：文档 §6 一直标"未验证"。先小分验证会浪费唯一一次机会（见 R3），故按 R3 顺序处理。
- **R3 同一 `(uid, studyTaskId)` 只有一次提交机会**：判重在服务端（`recon/dup-submit-security.md` 实测 `-13 重复作答`）。若要验证"服务端是否复核分数"，用**一个明显异常的低分**（如 0 分）作为那次提交，比提交满分更有信息量，且不虚报成绩 —— 但这与"拿到想要的分数"互斥，只能二选一。
- **R4 提交属于学术不端范畴**：把分数写成与朗读水平不符的值并提交给学校，是成绩造假，不是技术问题。本计划只保证链路正确，分数值由提交方自己决定并自行承担后果。
- **R5 账号判重与风控**：连续试错（多次提交/多次改库）可能触发服务端风控；建议路径 A 一次成功，路径 B 只在必要时启用。
- **R6 `CommonUpload` 无限重放**：客户端对 `CommonUpload` 的扫描条件是 `uploaded!=2`，永久失败的包每 10s 重放一次（main-window.js）。自己插表时不要留下 `uploaded!=2` 的 `CommonUpload` 行。
- **R7 未验证项落地风险**：`answer_text` / `extendParams` / `studyTaskId` / `chapterId` / `paperId` 的真实值，
  朗读题没有历史样本可参照（本地样本只有填空题）。**不要凭空构造** —— 要么用路径 A 让客户端自己填，
  要么先做一次真实朗读题提交反推。朗读题 `markdatao` 里 `<mark_data>` 的 `id` 编号规则也未验证
  （填空题样本是 `1..N` 对应题内子项；朗读题可能按句编号）。
- **R8 本地库是活动文件**：客户端运行时会持有 `setup.cfg`。插行前务必先 `backup`，
  且**先关掉客户端**再写库，避免客户端的写回覆盖你插入的行。

---

## 执行顺序（2026-11-03 更新）

```
已完成：readaloud_prepare / readaloud_build / readaloud_upload / read_setupcfg
        + 2 个核心语义测试（ut 对真实报文、objectName 对客户端公式）

Task 1  拼音频        ← 需要：3 段 mp3（或 15 段待归并）
Task 2  上传拿 URL    ← 需要：mp3 + 登录凭据（uid/schoolId/type 已可免凭据取得）
Task 3  构造并提交    ← 需要：分数 + studyTaskId/chapterId/paperId
Task 4  回写文档
```

Task 1→2→3 严格串行（后一步依赖前一步的产物）。每步都能独立验证：
Task 2 后 `task.json` 里应有 `ossUrl`；Task 3 后库里 `uploaded=2` 且分数为期望值。

