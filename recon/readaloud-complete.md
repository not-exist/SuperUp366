# 逐句跟读（Unit 4 / 课文1）—— 任务 · 引擎 · 提交格式 完整技术参考

> 目标作业：**Unit 4 JOURNEY ACROSS A VAST LAND / 课文1 / 逐句跟读**，截止 2026-11-10 00:00
> 本文是自洽参考：任务数据 + ASES 引擎调用 + 提交格式，三部分合在一起。
> 详细背景见 `recon/FINAL.md`（整体）、`recon/api.md`（全部接口）、`recon/ases.md`（评测引擎 423 行）。

---

## 0. 速览

```
题目(服务端)                 本地包(已下载)              本地评测                 提交
─────────────────────────────────────────────────────────────────────────────────────────────
章节树缓存                resources/<fileId>/            libases.node            TaskStuded / TestStuded
  └ pages[]               ├ paper.xml.u3enc            (utilityProcess)          → /client/task/score/submit/v2
    └ name=Unit 4…/逐句跟读   ├ correctAnswer.xml.u3enc     ├ 识别 parta/final.mdl   (带 ut, 课本分支)
      ├ fileId              ├ <elId>/media/*.mp3          ├ svm_accuracy_*            或
      ├ paperId             ├ <elId>/net/NNNN.net        ├ svm-fluency             /client/onlinetesting/submit
      ├ score=30            └ <elId>/net/psdata/         └ lightgbm_paragraph_      (无 ut, 测评分支)
      └ extendParams             answer.json                recite_* (5 档)
        = ListenAndReadv1
```

三个最容易踩的点：

1. **mp3 不是提交格式**。音频要先注册成 `RecordFiles` 行 → OSS 直传 → 拿到 OSS URL；提交行本身不含音频。
   音频与服务端的真实关联有**两条**，互不相同：
   - **服务端看到的**：`result.testInfo.answerdata`（朗读题 XML）里的 `<answer_url>`，以及 `qstJson[].elementAttr.record_url`。
   - **客户端本地队列**：`RecordFiles.taskId == 成绩行.taskId`（`df` 构造时按 taskId select）。这条只管"谁负责把文件传上 OSS"，服务端不参与。
   `TaskStuded` / `TestStuded` 两张表**没有 `uuid` / `objectIds` 字段**（只有 `CommonUpload` 有），所以别指望用它们引用音频。
2. **分数是客户端本地算的**（`AsesEngine.getResult()`），服务端只接收结果；`ut` 不是签名、`Authorization` 是死头。
3. **朗读题的"答案"就是学生本人的录音**。本文只做结构还原与本地自测，不做代打/伪造分数。

---

## 1. 任务与本地包

### 1.1 定位（来源：`C:/Up366StudentFiles/resources/cache/tree/9962655BB56EE1AD4128EFE0251D16B7.json`）

| 字段                             | 值                                                                         |
| ------------------------------ | ------------------------------------------------------------------------- |
| name                           | `Unit 4 JOURNEY ACROSS A VAST LAND/课文1/逐句跟读`                              |
| fileId                         | `2115269800157515843`（同名第二条变体：`2115270083101069405`）                      |
| paperId                        | `1D758BD86E8BE3AF0797789D68EF531B`（变体 `1EC280868E8EBB8FC9E67C60AAAA24C8`） |
| id / parentId                  | `3D1B42C891F7080656123366A77DD5A7` / `C3AE34CAD3D22AA92AD727A6269B0ECB`   |
| score / testDuration           | **30** / 303（变体 306）                                                      |
| pageType / type / displayOrder | 0 / 1 / 658（变体 659）                                                       |
| extendParams.extend4           | `{"openType":"ListenAndReadv1"}` → 阅读器以**听读模式**打开                         |

同名两条、fileId 与 paperId 都不同 = 同一作业的两个变体（不同班级/批次）。

### 1.2 包结构

`C:/Up366StudentFiles/resources/2115269800157515843/`

```
paper.xml.u3enc            题目结构（3 个 element）
correctAnswer.xml.u3enc    答案（朗读题，<answers>/<analysis> 均为空）
files.list                 包内清单 + md5 + size + mtime
version                    版本戳
<elementId>/media/U4RTR1-ZC.mp3          整段课文录音
<elementId>/net/NNNN.net                 逐句参考音频（自定义容器，magic = "VOASp"）
<elementId>/net/psdata/answer.json       ASES 评测输入
```

`.u3enc` = 16B IV + AES-128-CBC + PKCS#7，密钥 `QJBNiBmV55PDrewyne3GsA==`（硬编码于 up366.exe@128158069）。
解密：`node tools/u3enc.js decrypt <in.u3enc> <out>`。

### 1.3 题目分解

| 题   | elementId                          | 题型     | qlib 类型 | 分值   | 时长(s) | 句数  | 参考音频     |
| --- | ---------------------------------- | ------ | ------- | ---- | ----- | --- | -------- |
| Q1  | `35022420E8C14F9B94FC490E457EDED9` | 9 口语跟读 | 朗读      | 10.0 | 82    | 4   | 4 × .net |
| Q2  | `E58008A83034E394A4665B5FAD7138EE` | 9 口语跟读 | 朗读      | 10.0 | 86    | 5   | 5 × .net |
| Q3  | `666A4C22F65F8DDB243A8583ABE1AA7B` | 9 口语跟读 | 朗读      | 10.0 | 135   | 6   | 6 × .net |

合计 **30 分 / 15 句**，与树缓存 `score: 30` 一致。

- `question_type=9` = 口语跟读（引擎常量 `SPOKEN_READ`，xot-pfs `questionType` 表）
- `qlib_qst_type=351`、`qlib_qst_type_name=朗读`
- `element type="3"`、`question_kind=1`
- 逐句内容（原文/译文/对应 .net）见 `recon/task-unit4-readaloud.md` §4

### 1.4 ASES 评测输入契约（answer.json）

每题 `<elId>/net/psdata/answer.json`：

```json
{
  "Type": "parta", "SubType": "ReadAloud", "QuestionType": "Repeat",
  "QuestionTypeV3": "Paragraph", "ResponseMode": "ReadAloud",
  "DecoderType": "FA", "LearnPeriod": "JuniorHigh", "ConfusedWord": 0,
  "Data": {
    "sentences": [{
      "text": "SEEING THE TRUE NORTH VIA RAIL: VANCOUVER AND THE HEART OF CANADA ",
      "words": ["SEEING","THE","TRUE","NORTH","VIA","RAIL","VANCOUVER","AND","THE","HEART","OF","CANADA"],
      "sense_group": [0,0,0,0,0,2],
      "punctuation_index": [0,0,0,0,0,1,0,0,0,0,0,0],
      "punctuation_type": [": "],
      "special": [], "sent_longsil_num": 0
    }]
  }
}
```

- `Type=parta` → 识别模型 `model/recognition/parta/{final.mdl, phones.text}`
- `QuestionTypeV3=Paragraph` → 段落朗读打分 `model/score/readaloud/lightgbm_paragraph_recite_{extremely_strict,strict,normal,lenient,extremely_lenient}.model`

### 1.5 `.net` 容器

前 4 字节 magic `VOASp`，其后是长度/索引字段，再往后是高熵负载。**格式未完整逆**（未做样本比对），
但引擎只把它当**参考答案/参考音频**喂给 `ases.DataIn("netfile", path)`，不解析其内部结构。

---

## 2. 怎么调用引擎

### 2.1 进程模型

```
渲染层 ── Web Worker(Blob) 录 PCM→Int16、写 WAV ──┐
                                                   ├→ MessagePort ─→ utilityProcess.fork
主进程 ── worker_threads.Worker（主进程侧录音） ────┘                    │
                                                                    Zi（引擎包装）
                                                                      │
                                                        libases.node(N-API) → lsx-ases.dll
```

- 录音 Worker：`index.js@73556`（class `cr`）
- utilityProcess 桥：`index.js@75135`（class `hr`）
- 引擎包装：`index.js@90799`（`init(addonPath, modelDir, wcconfigDir, asesLog, addonLog)`）
- 信封：`{method, args, sync}` → `{sync, data}`（或 `{sync, error}` / `{code:-1,message}`）

### 2.2 插件加载（宿主侧）

```
PluginManager.init()      扫 <resources>/plugins 与 <saveDir>/plugins，按 config.json 版本去重
PluginManager.load("ases")
   └ index.js.u3enc 存在 → AES 解密 → vm.runInThisContext 包成 CJS → 返回 module.exports.default
调用方固定写 (await PluginManager.load("ases"))?.inst   → 插件必须提供 static get inst()
```

- 导出：`module.exports = { AsesEngine, default }`，两者同一个类 `Gn`（`index.js@94654`）
- `Gn extends EventEmitter`；事件：`recording` / `recordstoped` / `score`(`{code,data|message}`) / `recorderror` / `fft`(`[bars],quality,wavcheckJson`)

### 2.3 RPC 契约（插件 ↔ 引擎进程）

| method           | 参数                                                            | 返回               | 说明                                                                                                          |
| ---------------- | ------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------- |
| `init`           | `addonPath, modelDir, wcconfigDir, asesLogPath, addonLogPath` | `{code,message}` | `require(addon)` → `new AsesEngine(addonLog)` → `Start(modelDir)`；再 `new WavChecker` → `Start(wcconfigDir)` |
| `reInit`         | 同上去掉 addonLog                                                 | `{code,message}` | `code==4` 时重启                                                                                               |
| `startParse`     | `opts`（见 2.4）                                                 | `{code,message}` | `StartSession` + 装载 net/psdata + 可选 `SpeechIn("wavfile")`                                                   |
| `addData`        | `ArrayBuffer`(Int16 PCM)                                      | —                | `ases.SpeechIn("wav",buf)` + `wavChecker.SpeechIn("wav",buf)`                                               |
| `getCheckResult` | `true=speech_end / false=speech_in`                           | `{code,data}`    | 音质/VAD（`Realtime.Volume`）                                                                                   |
| `getResult`      | —                                                             | `{code,data}`    | `ases.Result()` → `JSON.parse`                                                                              |
| `stopParse`      | —                                                             | —                | `ases.StopSession()` + `wavChecker.StopSession()`                                                           |

错误码：`init` 返回 `code<0` 引擎启动失败 / `==1` 引擎 OK 但音质检测初始化失败（降级）/ `>1` 引擎启动失败；`startParse` 返回 `4` 表示需重启。

### 2.4 原生 addon API（`libases.node`）

```js
const y = require(addonPath);          // → { AsesEngine, WavChecker }（PE 导出表为 0，走 N-API 注册）

const ases  = new y.AsesEngine(addonLogPath);
ases.Start(modelDir);                  // 0 = ok，否则 GetError(code)
ases.SetAsesParam("log-open", logPath);
ases.SetAsesParam("log-level", "2");

const checker = new y.WavChecker;
checker.Start(wcconfigDir);            // 失败只降级，不致命

// 会话与参数
ases.StartSession(["parta","partb","partc","ASR"][type]);
ases.SetParam("md5", "1");
ases.SetParam("score_factor", factor); // 严格度系数 → 选档位模型
ases.SetParam("fake_net", x);           // 调试用

// 输入
ases.DataIn("netfile", netPath);        // type 0/1，每个 net 一次
ases.DataIn("path", psdataDir);         // type 0/1/2
ases.SpeechIn("wavfile", wavPath);      // 离线整文件
ases.SpeechIn("wav", int16Buffer);      // 流式

// 输出
const json = ases.Result();             // JSON 字符串
ases.GetError(code);                    // 错误码 → 中文原因
ases.StopSession();
```

### 2.5 `startParse` / `parseByNodejs` 的 opts（唯一输入契约）

| 字段                   | 类型       | 含义                              |
| -------------------- | -------- | ------------------------------- |
| `wav`                | string   | 已有 wav 路径；空串 = 实时流模式            |
| `type`               | 0/1/2/3  | → `parta / partb / partc / ASR` |
| `nets`               | string[] | `.net` 参考文件路径（type 0/1）         |
| `psdata`             | string   | 评测参数目录（本题即 `<elId>/net/psdata`） |
| `factor`             | string   | 严格度系数 → `score_factor`          |
| `fakeNets`           | string   | `fake_net` 调试开关                 |
| `realScoring`        | bool     | false 时直接返回（只录音/只做音质检测）         |
| `vad`                | bool     | 开 `WavChecker.StartSession`     |
| `fft`                | number   | 频谱柱数（12/20…），只影响 UI             |
| `jsCall` / `newAsec` | —        | **完全未被消费**（API 漂移遗留）            |

### 2.6 音频格式（采集链）

```
getUserMedia → AudioContext({sampleRate:16000}) → ScriptProcessor(4096)
  → 线性插值重采样 → Int16LE → worker → 主线程 → addData
  → 落盘：44 字节 WAV 头 + 逐块 writeSync（RIFF/fmt/data 手工拼，bitsPerSample=16）
```

| 参数      | 值                                                                                           |
| ------- | ------------------------------------------------------------------------------------------- |
| 采样率     | **16000 Hz**（AudioContext 强制 16k；不等时线性插值兜底）                                                 |
| 位深 / 声道 | **16 bit signed LE** / **默认 1**                                                             |
| 分片      | **4096 帧** = 256 ms                                                                         |
| 幅度      | `sample * 32767 * scale`，`scale` 默认 **0.333333**，可由 `appConfigs.recorder.scale` 覆盖（0.3~1.2） |
| 参考网     | `mictest/ce.net`（麦克风自测，parta）                                                               |

### 2.7 partType → 会话类型

| partType | 会话    | 用途                 |
| -------- | ----- | ------------------ |
| 0        | parta | 句子朗读 / 麦克风自测       |
| 1        | partb | 单词/句子跟读            |
| 2        | partc | 仅 psdata，无 netfile |
| 3        | ASR   | 开放题（口语/翻译/复述）转写为主  |
| 4        | → 3   | 归一化                |

### 2.8 打分链路

```
StartSession(parta) → Kaldi 在线解码（MFCC → HCLG 搜索 → 词/音素对齐）
  ├ 识别：model/recognition/parta/{final.mdl, HCLG.fst, phones.text}
  ├ 准确度：model/score/readaloud/svm_accuracy_{strict,normal,lenient}.model
  │          model/score/readaloud/accuracy_svm_junior_sentence_{strict,normal,lenient}.model
  ├ 流利度：svm-fluency.model / svm-word.model
  └ 段落朗读（本题）：lightgbm_paragraph_recite_{extremely_strict..extremely_lenient}.model（5 档）
```

### 2.9 最小离线打分序列（喂自己的 wav）

```js
// 需 32 位 Node/Electron（插件是 ia32/win32）
const p = 'C:/Program Files (x86)/up366/client/resources/plugins/ases/ia32/win32';
const y = require(p + '/libases.node');
const ases = new y.AsesEngine(null);
ases.Start(p + '/model');
ases.StartSession('parta');
ases.SetParam('md5', '1');
ases.DataIn('netfile', '<elId>/net/0001.net');     // 每个参考 net 一次
ases.DataIn('path',    '<elId>/net/psdata');
ases.SpeechIn('wavfile', '<你的录音>.wav');
const r = JSON.parse(ases.Result());               // → { code, data }
```

> 这条路径**只读本地文件、不联网、不提交**。具体到本题，`nets` 该传哪几个 .net（逐句 15 个，还是题目级 1 个）**尚未实测**，见 §6 待验证项。

---

## 3. 录音登记与上传

### 3.1 `RecordFiles` 表（本地 SQLite `setup.cfg`）

schema（表定义见 main/index.js@3525221 起的表结构对象；列名在 main-window.js@3815865 可见）：

| 列                          | 说明                                   |
| -------------------------- | ------------------------------------ |
| primary                    | (pageId, taskId, recordId)           |
| id                         |                                      |
| pageId / recordId / taskId | 录音归属：题页 / 录音 / 任务                    |
| bookId                     |                                      |
| **recordFile**             | 本地录音文件路径（wav）                        |
| **objectName**             | OSS 对象名（**本地生成**，见 §3.2）             |
| srcType                    |                                      |
| studyTime                  |                                      |
| fileSize                   |                                      |
| uploaded                   | 0 待传 / 1 上传中 / 2 成功 / -1 可重试 / -2 放弃 |
| failed / msg               |                                      |

### 3.2 上传

- `uploadRecord`（main-window.js@4128282 附近）：wav → **mp3** 转换（`recordFile.replace(/wav$/i,"mp3")`）→ `smartUploadFile`。
  **只有 `recordFile` 以 `.wav` 结尾才走这条转换路径**；不以 `.wav` 结尾的文件走 `uploadFile`（main-window.js@4131959 附近），
  **原样上传**（不做转码），成功后删除本地文件并置 `uploaded=2`。
- **`objectName` 是本地纯函数算出来的，不是上传后回填**：`buildOSSObjectName(e,t,r)`（index.js@3556173）
  ```
  storage{userType}/{scope}/{extClass}/{yyyy/mm/dd}{uid}[/{cloud}]/{fileName}
  // scope="study", extClass 由扩展名映射（mp3/wav/aac/flac/ogg/wma/m4a/amr → "a"）
  // userType=CurrentUser.type, uid=CurrentUser.uid, cloud="minicloud"（私有云开启时）
  ```
  录音登记时（`startRecord` 内）就已确定并写入 `RecordFiles.objectName`。
- 桶凭据：`/front/aliyunoss/token/v2?organId={schoolId}` 取 STS（`getOssTicket`，index.js@3577130），`ossBucketInfo.currentTime` 缓存 60 s。
  上传用 `aliyun-oss` 的 `multipartUpload(objectName, file)`（`da.upload`，index.js@3555880）。
- 私有云开启时改走 `/minicloud/upload`（multipart：bucketName / endpoint / objectName / fileData）。
- `recordFile.json` 与 mp3 一起上传（前者是元数据）；仅 `uploadRecord`（wav 路径）会带。
- md5 校验失败 → 删文件 → 回源重试一次 → 仍失败报 `MD5校验失败`。
- 下载落盘名 `{md5 ?? md5(url)}.data.u3temp`，全程 `Range: bytes={已有大小}-` 断点续传。

### 3.3 其它本地表（同一 SQLite）

| 表                 | 用途                                                       |
| ----------------- | -------------------------------------------------------- |
| `TaskStuded`      | 课本任务待交（见 §4.2）                                           |
| `TestStuded`      | 在线测试/作业待交（见 §4.3）                                        |
| `CommonUpload`    | 通用文件待传（primary: taskId；uuid / objectIds / requestParams） |
| `RecordFiles`     | 录音文件                                                     |
| `StudyTaskRecord` | 学习记录                                                     |
| `FlipbookAnswer`  | 翻页答案                                                     |
| `TaskScore`       | 本地分数簿记                                                   |
| `FileOSSURLMap`   | guid → filePath/objectName                               |

> 库是 **SQLCipher**，key = `"{3E2A243F-7B8B-45C8-9E7D-885A891820A7}"` **+ 用户名**（硬编码，main/index.js@3529280 的 `rekey="${LZ}${t??"}"`）→ 加密形同虚设。
>
> **两个易踩的细节**：
> 1. **per-user 库与公共库不是同一个文件**：登录后当前用户的数据在 `C:/Up366StudentFiles/user/<username>/setup.cfg`（key 拼用户名）；
>    `C:/Up366StudentFiles/setup.cfg` 是未登录时的公共库（key 只有常量段）。查历史成绩要查前者。
> 2. **表名全小写**（`taskstuded` / `recordfiles` / `commonupload` …）。用大写名查会得到"表不存在"，而不是空结果。

---

## 4. 提交格式

### 4.1 三条分支与判据

| 分支      | 表            | 接口                                    | ut    | 适用                                |
| ------- | ------------ | ------------------------------------- | ----- | --------------------------------- |
| A 课本    | `TaskStuded` | `POST /client/task/score/submit/v2`   | 有     | 课本任务（bookId + taskId + chapterId） |
| B 测评/作业 | `TestStuded` | `POST /client/onlinetesting/submit`   | **无** | 试卷类（带 paperId）                    |
| C 批量    | `CommonUpload` | `POST /client/task/score/gzip/submit` | 有     | 带附件的批量补交（`_SCHOOL_BOOK_PRACTICE_` 场景走这条） |

**本题判据**：条目带 `paperId`、`pageType=0` → **倾向分支 B**；最终看本地 `qstJson` 里是否出现 `testId`（B 会把 `taskId` 复制成 `testId`）。**未实测**。

> **实测补充（2026-11-03）**：本机 `C:/Up366StudentFiles/user/<账号>/setup.cfg` 里存着两条**真实课本提交报文**
> （`commonupload` 表，scenario = `_SCHOOL_BOOK_PRACTICE_`），走的是**分支 C（gzip）**，不是 A 也不是 B。
> 两条报文都是**填空题**（`question_type=4`），`bookId=9962655BB56EE1AD4128EFE0251D16B7`、`courseId=9222783`、`submitType=8`、
> `percent=97/100`、`score=0`。朗读题（`question_type=9`）的历史样本没有。
>
> 分支 A 的行也在同一库里（`taskstuded` 表 2 行，其中 1 行 `uploaded=2` 已提交），
> 形态是 `srcType=1`、`result=""`、`qstJson="{}"` —— 说明**至少存在一种"result 为空、分数走 qstJson"的课本提交形态**。
>
> **结论**：本题走哪条分支**仍未实测**，但三条分支的行字段与 XML 装法已全部拿到（见 §4.5），可直接照抄。

### 4.2 分支 A：课本任务（`/client/task/score/submit/v2`）

请求体（`application/x-www-form-urlencoded`，对象字段会被 `JSON.stringify` 后作为单个表单值）：

| 字段           | 类型            | 说明                                                      |
| ------------ | ------------- | ------------------------------------------------------- |
| `tasksJson`  | string        | **`JSON.stringify([row])`** —— 数组的 JSON 字符串，注意是"字符串套数组" |
| `submitType` | string/number | `e.submitType`                                          |
| `ut`         | string        | 见 §4.6                                                  |

`row`（`TaskStuded`）完整字段：

| 字段                                                | 来源                                                              | 说明                                             |
| ------------------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------- |
| `id`                                              | `pA()`                                                          | 随机 UUID                                        |
| `uid`                                             | `global.CurrentUser.uid`                                        | 学生 uid                                         |
| `bookId` / `taskId` / `chapterId` / `taskNo`      | 题目                                                              | `taskId` 取 `extendParams.practiceId ?? taskId` |
| `score` / `percent`                               | **渲染层给的分数**                                                     | 本地 ASES 算完后由页面回传                               |
| `studyDate`                                       | `AA().getTime()`                                                | 服务器时间（不是本机时间）                                  |
| `seconds`                                         | `duration`                                                      | 上报后立即归零，靠累加避免重复计时；为 0 时兜底 200                  |
| `result`                                          | `JSON.stringify({studentid, testInfo:{answerdata, markdatao}})` | **两份 XML 全部来自渲染层**，主进程不生成不校验                   |
| `srcType`                                         | 2（`submitAnswerData("textbook")`）/ 1（`submitTaskV1`）          |                                                |
| `batchTag`                                        | `pA()`                                                          | 随机 UUID                                        |
| `uploaded`                                        | 0                                                               | 本地簿记字段也会上报                                     |
| `studyTaskId` / `courseId`                        | 题目                                                              | `courseId` 被 `String()`                        |
| `extendParams`                                    | 对象或字符串                                                          | 字符串会被 `JSON.parse` 成对象后重新 stringify            |
| `qstJson`                                         | string                                                          | **按题粒度**，见 §4.5                                |
| `msg` / `failed` / `killStudyTask` / `submitType` |                                                                 | 本地簿记 / 业务字段                                    |

调用：`Ge.submitTaskScore(tasksJson, submitType)`（main/index.js@3579125）。

> **分支 A 与分支 C 的行字段不同**。分支 C（gzip，`submitTask` @index.js@3827639）的行是**远端 flip SDK 拼的**，
> 字段集实测为：`killStudyTask / batchId / bookId / studyTaskId / courseId / taskId / chapterId / paperId /
> bookType / seconds / percent / score / extendParams / taskNo / qstJson / deviceType / clientId / studyDate /
> wrongnoteFlag / uid`（**没有** `id` / `srcType` / `result` / `batchTag` / `uploaded` / `msg` / `failed`）。
> 分支 C 的 XML 放在 **`extendParams.xmlResult.{answerdata, markdatao}`**，不是 `result` 字段。


### 4.3 分支 B：测评/作业（`/client/onlinetesting/submit`）

请求体 = `TestStuded` 整行**直接透传**（没有 `tasksJson` 包装、没有 ut）：

| 字段                                          | 说明                         |
| ------------------------------------------- | -------------------------- |
| `uid`                                       | 学生 uid                     |
| `taskId`                                    | 测试任务 id（出队时会被复制成 `testId`） |
| `paperId`                                   | 试卷 id                      |
| `answerData`                                | 作答 XML（`userAnswerXmlStr`） |
| `duration`                                  | 时长（默认 1）                   |
| `beginTime` / `endTime`                     | `startTime` / `submitTime` |
| `playData` / `timeData`                     | 播放与计时轨迹                    |
| `uploaded` / `studyDate` / `failed` / `msg` | 簿记                         |

调用：`Ge.submitTestScore(row)`（main/index.js@3579458）。
"homework" 分支的 `submitAnswerData` 构造见 main-window.js@4051232。

### 4.4 分支 C：gzip 批量（`/client/task/score/gzip/submit`）

```
POST https://study-api.up366.cn/client/task/score/gzip/submit
Content-Type: application/json;charset=UTF-8
Content-Encoding: gzip

{ "tasksJson": "<JSON string>", "ut": "<...>", "submitType": <...> }
```

- 附件先打包成 zip（`mE` 类，7za 或内置实现），再整体 gzip；`urlList` 里每个 URL 取 `new URL(u).pathname.slice(1)` 作 objectName，去重后 join(",") 存 `objectIds`。
- 音频本体走 `smartUploadFile`（OSS/私有云），**不在这两个接口里**。
- `ut` 用 `buildUT(tasksJson, "submitTaskToken")`，盐与分支 A 一致。

**实测的真实 body（2026-11-03，从本地 `commonupload` 表现存报文还原）**：

```jsonc
{
  "tasksJson": "[{ ...单行... }]",   // 字符串套数组
  "ut": "f7a05c3ed554d6b8fd629012ae96a1316f938626d2711367dd2d3a1aab73b5d0",
  "submitType": 8
}
```

单行（分支 C 形态，与分支 A 不同）：

```jsonc
{
  "killStudyTask": 0, "batchId": "6cf53464c0e8438b8e38cdff6b0d4fb",
  "bookId": "9962655BB56EE1AD4128EFE0251D16B7",
  "studyTaskId": "4578C972D39EE268F750C3CAB5EB6DCE", "courseId": 9222783,   // = 作业列表的 jobId，见 §6
  "taskId": "A2E3A3C6735D4FE3043BFD2490AEFEDD", "chapterId": "A2E3A3C6735D4FE3043BFD2490AEFEDD",
  "paperId": "56D60FCB8C8B79B0E05353E1A48B8B45", "bookType": 2,
  "seconds": 1771, "percent": 97, "score": 0, "taskNo": 1,
  "deviceType": "Win10", "clientId": "7057119429F1E0F7138F82EFF5BFE50D270DD095",
  "studyDate": 1787299639021, "wrongnoteFlag": "PRODUCT", "uid": 39370562,
  "extendParams": { "batchId": "...", "xmlResult": { "answerdata": "<elements>…</elements>", "markdatao": "<elements>…</elements>" } },
  "qstJson": [ /* 见 §4.5，数组 */ ]
}
```

`ut` 长度 **64**（`r.slice(0,10) + 32 + r.slice(10)`），盐 `submitTaskToken` → 实际串为 `md5(ts)[:10] + md5(md5(payload)+"submitTaskToken-pc-987654"+md5(ts)) + md5(ts)[10:]`。

### 4.5 `qstJson` —— 真正的"按题"结构

**两种形态，取决于分支**：

**分支 A（`/client/task/score/submit/v2`）**：DB 里存的是对象，键为 **`<elementId>_<order>`**：

```js
// addTaskStuding(bookId, taskId, list)
for (const s of list) n[`${s.elementId}_${s.order}`] = s;
A.qstJson = JSON.stringify(n);
```

- 值 = 该题的作答对象（分数等业务字段）。
- 上传前**拍平**：`Object.entries(qstJson).map(x => x[1])` → 数组 ⇒ **键名对服务端无意义**，只保序。
- `extendParams` 相反：字符串会被 parse 成对象再发出去。

**分支 C（gzip）**：`qstJson` **本身就是数组**（无键），元素形态实测为：

```jsonc
{ "elementId": "A37CE46132C0CADCD4E62E6C772CF2FD", "elementType": 1, "elementAttr": { … } }
{ "elementId": "A37CE46132C0CADCD4E62E6C772CF2FD-1", "elementType": 2, "elementAttr": { … } }
```

- `elementType`：**1 = 题（QUESTION）**，**2 = 题内子项（SUB_QUESTION）**。子项的 `elementId` = 题 id + `-` + 序号。
- `elementAttr` 字段全集（实测 280 条样本统计）：
  `question_id / question_type / order / user_score / user_answer / answer / score / result / timestamp /
  comment_ai / record_url / duration / wrongnote_flag / recForm / comment / pic_url / ocrTids`
  （`record_url` / `duration` / `wrongnote_flag` / `recForm` / `comment` / `pic_url` / `ocrTids` 只在子项上出现）
- **朗读题（`question_type=9` = `SPOKEN_FOLLOW_READ`）的 `elementAttr` 额外字段**（来自远端 flip SDK `markSpokens`）：
  题级：`user_score / result / record_result / timestamp / record_url / duration / accuracy / completion / fluency / code`
  句级（子项）：`user_score / user_answer / record_result / timestamp / record_url / accuracy / completion / fluency`
  - `record_url` **只有题级一个**（整题录音的 OSS URL），句级 `record_url` 复用同值。
  - `result` = `getResultByEngineScore(引擎分 × 100 / questionScore)`，取值 `1` 全对 / `2` 错 / `3` 部分对（`MarkResultType`）。
  - `accuracy` / `completion` / `fluency` 是引擎给的三个维度分。

**XML 与 qstJson 的对应关系**（实测对齐）：

| 来源 | 结构 |
| --- | --- |
| `answerdata` | `<elements>` + 每题一个 `<element id="题id"><answers>…</answers></element>`；题内子项在 `<answers>` 内按 `<answer id="N">` / `<mark_data id="N">` 编号 |
| `markdatao` | `<elements>` + 每题一个 `<element id="题id">`，题内 `<mark_data id="N"><score/><result/>…</mark_data>` |
| `qstJson` | 每题 1 条 `elementType=1` + N 条 `elementType=2`（`elementId-N`） |

⇒ **`answerdata` / `markdatao` 的 `<element>` 数量 = `qstJson` 里 `elementType=1` 的数量**（题数），题内子项靠 `id` 对齐。


### 4.6 公共传输约定

固定 7 个头（`request-created` 钩子统一注入）：

| Header             | 值                              |
| ------------------ | ------------------------------ |
| `user-agent`       | `PC-Up366-Student 6.11.0`      |
| `x-requested-with` | `PC`                           |
| `X-App-Name`       | `student-pc`                   |
| `clientId`         | `MachineCode`（主板/BIOS/OS 组合指纹） |
| `u3r`              | 每请求一个 `randomUUID()`（去横线）      |
| `u3t`              | **TGT**（不是 ST）                 |
| `Authorization`    | **恒为空串**（死头，全包无赋值点）            |

- 体编码：默认 `x-www-form-urlencoded`；对象/数组值 `JSON.stringify`；键里的 `###` 换成 `.`。
- 无请求体加密、无请求体压缩（仅 gzip 通道）。
- **ST 兜底**：返回 `result.code ∈ {-21,-22}` → `POST /v1/serviceTicket {service:<原完整URL>}` → 拿到 `data.serverTicket` → 原 URL 追加 `?ticket=<st>` **重放一次**；再失败则用 token/openId/账密重登一次。
  细节（`getServiceTicket`，index.js@3565449）：**必须 `code == -24` 才算拿到 ST**；否则触发一次重登（token → openId → 账密）再取一次。
- `ut` 构造（各接口盐不同，常量全在 bundle 内，**离线可构造**）：

```js
// 分支 A / 分支 C 共用同一形式（实测 ut 长度 64）
const r = md5(String(Date.now()));   // 32 hex
const n = md5(md5(payload) + "<盐>" + r);
ut = r.slice(0, 10) + n + r.slice(10);
```

两种写法等价（`submitTaskScore` 内联版 与 `buildUT` 版）：

```js
// 分支 A（index.js@3579270）：盐写死
md5(md5(tasksJson) + "submitTaskToken-pc-987654" + r)
// 分支 C（index.js@3827639，走 buildUT）：盐作参数
md5(md5(tasksJson) + "submitTaskToken" + "-pc-987654" + r)
```

| 接口                                     | 盐                           | payload                                                                                    |
| -------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------ |
| `/v2/tickets-v3`                       | `loginEncryptionParameter`  | `username + md5(md5(密码)+"fa&s*l%$k!fq$k!ld@fjlk")`                                         |
| `/v2/auto/tickets`、`weChat/qq/tickets` | `loginEncryptionParameter`  | token / openId                                                                             |
| `/front/user/verify/send`              | `sendSMSToken`              | mobile                                                                                     |
| `/client/task/score/submit/v2`         | `submitTaskToken-pc-987654` | `md5(tasksJson)`                                                                           |
| `/client/task/score/gzip/submit`       | `submitTaskToken-pc-987654` | `md5(tasksJson)`（同上，盐拆成 `"submitTaskToken"` + `"-pc-987654"`）                        |
| 文件元信息                                  | —                           | 不是 ut，是 `HMAC-SHA1(key="au66ssoI3nGr6DxCHk8KcpVsQ1wYNQId", msg="GETOBJECTschool"+fileIds)` |

> `T = md5(String(Date.now()))` 不可逆 ⇒ 服务端**无法施加时效窗口**，ut 不构成签名。

### 4.7 出队与重试

```
SQLite 行 → df.uploader.start()
   ├─ 上传录音：wav → mp3 → OSS（objectName 事先已定，见 §3.2）
   ├─ TaskStuded → Ge.submitTaskScore(JSON.stringify([row]), row.submitType||"0")
   ├─ TestStuded  → row.testId = row.taskId → Ge.submitTestScore(row)
   └─ CommonUpload → 按 requestParams 逐个 POST（gzip 分支在此）
失败：result.code === -28 → uploaded = -1（可重试）
      其它非 0    → uploaded = -2（放弃）
成功：uploaded = 2，且 seconds 立即归零
```

- 启动时 `startUpload()` 扫表重放。
- **客户端缺陷**：`TaskStuded`/`TestStuded` 的扫描会跳过 `uploaded == -2`，但 `CommonUpload` 的条件只有 `uploaded != 2`，**永久失败的包每 10 s 重放一次**（main-window.js@3878614 附近）。

### 4.8 端到端时序

```
渲染层(阅读器)
  ├─ asesEngine.startRecord({type:0, nets:[...], psdata:'.../net/psdata', vad:1, fft:12})
  │    → getUserMedia 16k → addData(Int16) → 引擎
  │    → stopRecord → getResult() → {code, data}
  ├─ 组装 answerData / markData XML + 每题分数
  ├─ (可选) addRecordFile({pageId, taskId, recordId, recordFile})  → 主进程登记 + 上传
  └─ submitAnswerData('textbook'|'homework', _, JSON)      main-window.js@4051232
       ├─ textbook → uploadTaskScore(...) → TaskStuded
       └─ homework → uploadTestScore(...) → TestStuded
            └─ df.uploader.start() → 提交接口 → 服务端
```

### 4.9 朗读题的录音粒度与音频 URL 机制（2026-11-03 实测）

这是本题最关键、也最容易搞错的一节。

**录音单位 = 题，不是句。**

- 本地包里每题一个 `net/psdata/answer.json`，其 `Data.sentences[]` 含该题**全部句子**（Q1=4 句 / Q2=5 句 / Q3=6 句）；
  每句一个 `.net` 参考音（`net/0001.net`…）。`answer.json` 的 `QuestionTypeV3=Paragraph` → 段落朗读模型（`lightgbm_paragraph_recite_*`）。
- `paper.xml` 的朗读题 element：`question_type=9`、`<duration>` 82/86/135 秒（整题时长）、
  `extend2 = {"contentType":5,"paragraphCount":6,"parthaMode":2}`。
  `attachment` 是 URL 编码的 U3CompoLib 数据，解码后为 `<paragraph><sentences id="1">…<netfiles><item netfile="net/0001.net"/>` 结构，**句号在 sentences 里**。
- 远端 flip SDK 的 `markSpokens`（`work/report/u3_flip_sdk.4672f20a.js`）对 `SPOKEN_FOLLOW_READ`(=9) 的处理：
  **一次录音** → 引擎返回 `result.sentences[]`（引擎按 `.net` 参考音切句，每句带 `score`/`recResults`/`accuracy`/`completion`/`fluency`/`timestamp`）
  → 汇总成题级 `user_score` 与 `result`。
  ⇒ **15 句对应 3 段录音**（不是 15 段），句级分数由引擎在**一次**会话里切出来。

**音频 URL（`record_url`）的产生与流向**：

```
录音完成 → 主进程 startRecord 内 c():
   E = buildOSSObjectName("study", "<uuid>.mp3", cloud)     // 本地算，见 §3.2
   addRecordFile({... objectName:E, recordFile:<本地 wav 副本>, uploaded:0 })
   n = await getOssHost(E)   // → https://<bucket>.<host>/<E>
   emit "recorderstatechangev1" { recordUrl: n, duration, state }
        ↓
渲染层拿到 recordUrl → 写进题级 elementAttr.record_url
        ↓
提交时：extendParams 里的 urlList 收集各口语题的 record_url
        ↓
客户端 submitTask → mE.addFile(url) → objectName = new URL(url).pathname.slice(1)
   → CommonUpload.objectIds（附件清单，服务端据此关联 OSS 上的音频）
```

- `urlList` 的收集条件（远端 SDK `SubmitImpl.js`）：**只有 `question_type ∈ {9, 12, 13}`（口语跟读/问答/复述）且 `record_url` 存在时才 push**；
  `ASK_ANS`(7) 额外收集 `pic_url`。
- `objectIds` = 各 URL 的 `pathname.slice(1)` 去重后 `join(",")`，即 **OSS objectName 清单**。
- **录音本体不在提交报文里**：客户端先把文件传到 OSS（`smartUploadFile`），提交报文只带 URL/清单。
  ⇒ 自建提交时，**必须先确保 OSS 上确实存在这些 objectName**，否则服务端拿不到音频。

**`elementAttr` 里分数相关的字段语义**（朗读题）：

| 字段 | 层级 | 含义 |
| --- | --- | --- |
| `user_score` | 题 / 句 | 引擎给的原始分（题级 = 句级汇总） |
| `result` | 题 / 句 | `getResultByEngineScore(引擎分 × 100 / questionScore)` → `1` 全对 / `2` 零分 / `3` 部分对 |
| `record_url` | 题（句级复用同值） | 整题录音的 OSS URL |
| `record_result` | 题 | 识别文本的 HTML（`<span class="word" score="..">词</span>`） |
| `accuracy` / `completion` / `fluency` | 题 / 句 | 引擎三维度分 |
| `duration` / `timestamp` / `code` | 题 | 录音时长 / 打分时刻 / 引擎返回码 |

---

## 5. 端到端复现（自己的录音 → 本地分数 → 自己提交）

```bash
# 1) 解题包
node tools/u3enc.js decrypt "C:/Up366StudentFiles/resources/2115269800157515843/paper.xml.u3enc" work/task_unit4/paper.xml
python tools/task_dump.py > work/task_unit4/task.txt

# 2) 本地打分（只读本地音频）
ELECTRON_RUN_AS_NODE=1 "<up366.exe>" -e "$(cat tools/practice_score.js)"   # 见 §2.9

# 3) 提交（由你本人在客户端里完成；需要确认报文时挂 hook 只读一次）
```

---

## 6. 已验证 / 未验证

**已验证**

- 题目包结构、15 句内容与译文、answer.json 契约（全部本地实读）
- `.u3enc` 加解密 43/43
- 引擎 RPC 契约、addon 方法名、opts 字段、音频格式、partType 映射（静态实读）
- 提交字段表、`ut` 构造、ST 兜底、队列表与状态机（静态实读）
- **本地真实课本提交报文**（`commonupload` 表 2 条，gzip 分支，`submitType=8`）：行字段全集、`qstJson` 数组形态、`elementAttr` 字段全集（280 样本统计）、`elementType` 层级、`answerdata`/`markdatao` XML 形状、`ut` 长度 64
- **objectName 是本地生成**（`buildOSSObjectName`），非上传后回填
- **`TaskStuded`/`TestStuded` 无 `uuid`/`objectIds`**；`RecordFiles` 与成绩行靠 `taskId` 关联
- **SQLCipher key = 常量段 + 用户名**；per-user 库与公共库是不同文件；表名全小写
- **朗读题录音单位 = 题**（一次录音 → 引擎按 `.net` 切句 → 15 句 = 3 段录音）
- **`record_url` / `urlList` / `objectIds` 机制**：题级一个 OSS URL，`urlList` 只收口语题的 `record_url`，`objectIds` = URL 的 `pathname.slice(1)`

**未验证**

- `.net` 在 `nets` 里应传逐句 15 个还是题目级 1 个
- `.net`（VOASp）内部格式
- 朗读题 `markdatao` 里 `<mark_data>` 的确切 `id` 编号规则（填空题样本是 `1..N` 对应题内子项；朗读题未验证）

**2026-11-03 真实提交后新增（详见 `recon/readaloud-submit-verify.md` / `recon/readaloud-runbook.md`）**

- ✅ **本题走分支 A**（`POST /client/task/score/submit/v2`，`submitType=8`，XML 放 `result`）
- ✅ **`studyTaskId` = 作业列表的 `jobId`**（实测样本与 `job/info` 均验证）；自造 32 hex 能写成绩记录，
  但 job 的 `taskStatus` 恒为 0 ⇒ 所有客户端显示未完成
- ✅ **完成判据**：`job/info.taskStatus` 0→1、`submitTime` 非 0、`stuStatus=2`
- ⚠️ **服务端疑似异步复评分数**：上报 24，同一条记录（batchId 不变）稍后变成 8.2，
  `firstScore=8.2`、`percent` 仍 100 —— 未定位，按"自定义分数不可靠"处理

---

## 7. 风险与边界

- SQLCipher 密钥硬编码 → 本地"加密"等于明文；`Login.getUserInfo()` 还能被任意渲染进程直接读到**明文口令**。
- ASES 引擎是 ia32/win32 原生模块，独立调用必须 32 位运行时 + 完整 `model/`、`wcconfig/` 目录。

---

## 8. 文档索引

| 文档                                                               | 内容                             |
| ---------------------------------------------------------------- | ------------------------------ |
| `recon/task-unit4-readaloud.md`                                  | 本题的 15 句逐句内容（原文/译文/参考音频）       |
| `recon/api.md`                                                   | 全部 54 个接口 + 字段级说明 + 签名/登录态/错误码 |
| `recon/ases.md`                                                  | ASES 插件 423 行深度分析（评测链路、模型、特征）  |
| `recon/security.md`                                              | 客户端安全评估（7 高危 / 7 中危 / 4 低危）    |
| `recon/FINAL.md`                                                 | 总体结论                           |
| `docs/plans/2026-11-01-readaloud-submit-plan.md`                 | 可行性判定与实施计划                     |
| `tools/u3enc.js` / `tools/task_dump.py` / `tools/task_report.py` | 解密 / 题目结构导出                    |
