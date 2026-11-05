# Unit 4 / 课文1 / 逐句跟读 —— 专项逆向

> 目标作业：**Unit 4 JOURNEY ACROSS A VAST LAND / 课文1 / 逐句跟读**，截止 2026-11-10 00:00
> 证据全部来自本地已下载的题目包与章节树缓存，未联网、未提交。

## 1. 任务定位

来源：`C:/Up366StudentFiles/resources/cache/tree/9962655BB56EE1AD4128EFE0251D16B7.json`（章节树缓存）

| 字段 | 值 |
|---|---|
| name | `Unit 4 JOURNEY ACROSS A VAST LAND/课文1/逐句跟读` |
| fileId | `2115269800157515843`（另一变体 `2115270083101069405`） |
| paperId | `1D758BD86E8BE3AF0797789D68EF531B`（变体 `1EC280868E8EBB8FC9E67C60AAAA24C8`） |
| score | **30** |
| testDuration | 303（变体 306） |
| pageType / type | 0 / 1 |
| displayOrder | 658（变体 659） |
| extendParams.extend4 | `{"openType":"ListenAndReadv1"}` |

树里同名条目有 **2 条**，fileId 与 paperId 都不同 —— 同一份作业的两个变体（不同班级/批次）。

`openType=ListenAndReadv1` 表示阅读器以**听读模式**打开，不是普通试卷。

## 2. 本地题目包结构

`C:/Up366StudentFiles/resources/2115269800157515843/`

```
paper.xml.u3enc            题目结构（3 题）
correctAnswer.xml.u3enc    答案（朗读题，answers/analysis 均为空）
files.list                 包内清单 + md5 + 大小
version                    版本戳
<elementId>/media/*.mp3    整段课文录音
<elementId>/net/NNNN.net   逐句参考音频（自定义容器，magic = "VOASp"）
<elementId>/net/psdata/answer.json   ASES 评测输入（句子 / 单词 / 意群 / 标点）
```

`.u3enc` 用 `node tools/u3enc.js decrypt <in> <out>` 直接解（AES-128-CBC + 前置 IV + PKCS#7）。

## 3. 题目分解

{{QTABLE}}

合计 **30 分 / 15 句**，与树缓存里 `score: 30` 一致。

## 4. 逐句内容

{{STABLE}}

## 5. ASES 评测输入契约（answer.json）

每题的 `net/psdata/answer.json`：

```json
{
  "Type": "parta", "SubType": "ReadAloud", "QuestionType": "Repeat",
  "QuestionTypeV3": "Paragraph", "ResponseMode": "ReadAloud",
  "DecoderType": "FA", "LearnPeriod": "JuniorHigh",
  "Data": {
    "sentences": [{
      "text": "SEEING THE TRUE NORTH VIA RAIL: ...",
      "words": ["SEEING", "THE", "TRUE", "NORTH", "VIA", "RAIL", "VANCOUVER", "AND", "THE", "HEART", "OF", "CANADA"],
      "sense_group": [0, 0, 0, 0, 0, 2],
      "punctuation_index": [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
      "punctuation_type": [": "],
      "special": [],
      "sent_longsil_num": 0
    }]
  }
}
```

- `Type=parta` → 识别模型 `plugins/ases/ia32/win32/model/recognition/parta/{final.mdl,phones.text}`
- `QuestionTypeV3=Paragraph` → 段落朗读打分模型 `model/score/readaloud/lightgbm_paragraph_recite_{extremely_strict,strict,normal,lenient,extremely_lenient}.model`（五档）

## 6. 评分与提交链路

```
录音 → 客户端编码 → 逐句 .net（VOASp 容器）
   → utilityProcess → libases.node → lsx-ases.dll
        ├─ 识别：parta/final.mdl + HCLG.fst + phones.text
        ├─ 准确度：svm_accuracy_{strict,normal,lenient} / accuracy_svm_junior_sentence_*
        ├─ 流利度：svm-fluency / svm-word
        └─ 段落朗读：lightgbm_paragraph_recite_*（5 档）
   → 每题得分（满分 10）→ 逐题写 qstJson（键 <elementId>_<order>）
   → TaskStuded / TestStuded 队列 → 提交接口
```

提交字段见 `recon/api.md` §1.14：

- 课本任务分支：`POST /client/task/score/submit/v2`，`tasksJson=[row]`，`result={studentid, testInfo:{answerdata, markdatao}}`，带 `ut`。
- 试卷/测评分支：`POST /client/onlinetesting/submit`，整行透传，无 `ut`。

本题带 `paperId` 且 `pageType=0`，**走测评分支的可能性更大**；判定依据是 `qstJson` 里是否出现 `testId`。

## 7. 复现

```bash
node tools/u3enc.js decrypt "C:/Up366StudentFiles/resources/2115269800157515843/paper.xml.u3enc" work/task_unit4/paper.xml
python tools/task_dump.py > work/task_unit4/task.txt
```

## 8. 边界

本文只做**结构还原**：题目、逐句文本、译文、参考音频、评测输入契约、提交字段。
不生成替代学生作答的音频，也不做自动刷分 —— 朗读题的"答案"本身就是学生本人的录音。
