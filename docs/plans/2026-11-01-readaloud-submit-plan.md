# 逐句跟读（Unit 4 / 课文1）提交链路 —— 可行性判定与实施计划

> 提问：我给你任意的 15 个 mp3，能否 submit 到远端？
> 结论先行：**机械上可行，但拿不到你想要的分数；而且我只会做"让你自己的录音在本地就能看到分数"这一段。**

---

## 0. 可行性判定（先把链路说清楚）

| 环节 | 在哪 | 事实 |
|---|---|---|
| 录音/解析 | `plugins/ases/index.js` `AsesEngine.startRecord` @95798、`parseByNodejs` @97820、`getResult` @95547 | 引擎在 **utilityProcess** 里跑，初始化时加载 `libases.node` + `model/` + `wcconfig/`；`parseByNodejs` 也能直接吃 wav 文件 |
| 打分 | 同上 + `model/score/readaloud/*` | **分数是客户端本地算的**：parta 识别 → 准确度/流利度 svm → 段落朗读 `lightgbm_paragraph_recite_*`（5 档） |
| 音频登记 | `RecordFiles` 表 | schema：primary(pageId, taskId, recordId)，另有 id / bookId / recordFile / objectName / srcType / studyTime / failed / uploaded / fileSize / msg |
| 音频上传 | `uploadRecord`（main-window.js@4128282 附近） | wav → mp3 转换 → OSS 直传（STS 来自 `/front/aliyunoss/token/v2`），回填 `objectName` |
| 提交行 | `TaskStuded`（课本，带 ut）或 `TestStuded`（测评，无 ut） | 见 `recon/api.md` §1.14 |
| 服务端 | `/client/task/score/submit/v2`、`/client/onlinetesting/submit` | **只接收客户端算好的分数**；是否另做复核**未验证** |

### 所以"任意 15 个 mp3 → submit"实际会发生什么

1. 客户端不会校验音频是不是现场录的，**音频确实可以被替换**；
2. 但分数不是你能填的 —— 它由本地 ASES 对**你提供的音频**算出来，读得不像就是低分；
3. 想指定分数只能绕过本地引擎直接伪造报文，那是**篡改评分**（服务端是否复核未验证），**这一段我不做**；
4. mp3 也不是直接提交的格式：音频要先注册成 `RecordFiles` 行、OSS 直传、拿到 `objectName`，提交行只带引用（`uuid` / `objectIds`）。

### 我愿意做的（对你 11-10 截止最有帮助）

把你自己的录音接进**同一个 ASES 引擎**，提交前就能看到逐句得分和维度分，练到达标再自己提交。
这复用客户端原引擎，不改分数、不伪造、不代打。

---

## 1. 实施计划

### P0 补齐三个待确认事实（0.5h，只读）

1. **题目侧**：15 句与 `pageId` / `recordId` 的对应关系 —— 从 `paper.xml` 的 element id + `answer.json` 的 `sentences[]` + `net/NNNN.net` 序号推。
2. **引擎侧**：`parseByNodejs` 的完整入参（opts 字段名、wav 采样率/位深要求），先用引擎自带样例跑通一次。
3. **提交侧**：本题走 `TestStuded` 还是 `TaskStuded` —— 看本地 `setup.cfg` 里两表的行；判据是 `qstJson` 是否含 `testId`。

### P1 本地打分闭环（1~2h，合法高价值）

1. 在 `ELECTRON_RUN_AS_NODE=1` 下直接加载 `libases.node`：`init(lib, model, wcconfig)` → `parseByNodejs({wav})` → `getResult()`。
2. 喂**你自己录的**音频，拿到逐句分数与维度（准确度 / 流利度 / 段落朗读档位），与客户端显示对齐。
3. 产出 `tools/practice_score.py`：只读本地音频、**不联网、不提交**。

### P2 逐句练习包（0.5h）

1. 导出 15 句的原文 / 译文 / 参考音频（`.net` → 可播放格式）到 `work/practice/`。
2. 本地 HTML 练习页：逐句播放参考音 → 显示文本 → 录音 → 回显本地得分。
   —— 这样你 11-10 前可以在本地反复练，用真实录音提交。

### P3 你自己提交时协助抓包（可选，仅在你操作时）

挂 hook 读一次 `answerData` XML 结构，确认测评/课本分支、字段与签名。
**只读不写，不代你提交。**

### P4 明确不做

代打、伪造分数、替换他人音频刷分。

---

## 2. 风险与前提

- ASES 引擎跑在 utilityProcess 里，独立调用需要正确的 arch（ia32/win32）与完整 `model/`、`wcconfig/` 目录；
- 录音 mp3 会传到学校 OSS（`smartUploadFile`），老师侧可见，别上传不想留存的音频；
- 服务端是否对提交的分数做二次复核：**未验证**（已发包验证才能确认，本计划不做）；
- 时间：P0+P1+P2 约 1 个工作日，够练到提交。

---

## 3. 待确认

1. 你是否已有自己录的 15 句音频（wav/mp3 均可）？—— P1 需要。
2. 要不要我先把 15 句参考音频导成练习包（P2 不依赖你录音，可先做）？
