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

| # | elementId | 题型 | 分值 | 时长(s) | 句数 | 参考音频 |
|---|---|---|---|---|---|---|
| 1 | 35022420E8C14F9B94FC490E457EDED9 | 9 口语跟读 | 10.0 | 82 | 4 | 4 个 .net |
| 2 | E58008A83034E394A4665B5FAD7138EE | 9 口语跟读 | 10.0 | 86 | 5 | 5 个 .net |
| 3 | 666A4C22F65F8DDB243A8583ABE1AA7B | 9 口语跟读 | 10.0 | 135 | 6 | 6 个 .net |

合计 **30 分 / 15 句**，与树缓存里 `score: 30` 一致。

## 4. 逐句内容

### 第 1 题（35022420E8C14F9B94FC490E457EDED9）

| # | 参考音频 | 原文 | 译文 |
|---|---|---|---|
| 1 | net/0001.net | SEEING THE TRUE NORTH VIA RAIL: VANCOUVER AND THE HEART OF CANADA | 坐火车看“真北方”：温哥华与加拿大腹地 |
| 2 | net/0002.net | Li Daiyu and her cousin Liu Qian went to Canada to visit their cousins in Halifax on the Atlantic coast. | 李黛予和表妹刘倩前往加拿大探望居住在大西洋海岸哈利法克斯市的表兄妹。 |
| 3 | net/0003.net | Rather than travel by commercial airline all the way, they decided to fly to Vancouver and then take the train. | 她们没有一路乘坐商业航班，而是决定先飞到温哥华，然后再乘坐火车。 |
| 4 | net/0004.net | For both of them, the thought of crossing the whole country by rail was exciting. | 一想到可以坐火车横穿整个国家，表姐妹俩就激动不已。 |

### 第 2 题（E58008A83034E394A4665B5FAD7138EE）

| # | 参考音频 | 原文 | 译文 |
|---|---|---|---|
| 1 | net/0001.net | Before starting out, they spent a couple of days in Vancouver, seeing the sights. | 出发前，她们在温哥华待了两天，欣赏当地风光。 |
| 2 | net/0002.net | During their first day, as is typical of Vancouver, it rained. | 第一天，温哥华一如往常细雨蒙蒙。 |
| 3 | net/0003.net | Despite the weather, they were able to take a boat ride out into the bay, and later visit an island that had wonderful shops selling crafts and antiques. | 尽管天气不佳，她们仍然泛舟驶入海湾，接着游览了一座岛屿。岛上有精致的店铺，出售手工艺品和古董。 |
| 4 | net/0004.net | The next day was clear and mild, and they were pleased to see the beautiful mountains looking out over the city. | 第二天，天气晴朗，温度适宜，她们看到壮美的群山俯瞰着整座城市，甚是欣喜。 |
| 5 | net/0005.net | Later, they took a pleasant hike in a forest just a short distance away. | 随后，她们在不远处的森林中徒步旅行，十分惬意。 |

### 第 3 题（666A4C22F65F8DDB243A8583ABE1AA7B）

| # | 参考音频 | 原文 | 译文 |
|---|---|---|---|
| 1 | net/0001.net | The next morning, the two girls arose early to take the train to Lake Louise, passing through the Canadian Rockies. | 次日清晨，两个女孩早早起床，登上开往路易斯湖的火车，穿越加拿大落基山脉。 |
| 2 | net/0002.net | Seen from the train window, the mountains and forests of Canada looked massive. | 从车窗往外望去，加拿大巍峨的山脉和辽阔的森林尽收眼底。 |
| 3 | net/0003.net | When the train arrived at the station, they took a taxi to Lake Louise, where the blue water literally took their breath away with its exceptional beauty. | 火车到站后，她们乘坐出租车前往路易斯湖。那里湛蓝的湖水美丽异常，摄人心魄。 |
| 4 | net/0004.net | They spent the night, and then took a coach bound north through the Canadian Rockies to Jasper. | 住了一夜之后，她们乘坐长途汽车，向北穿过加拿大落基山脉，直达贾斯珀。 |
| 5 | net/0005.net | Looking at the beautiful scenery, they both agreed that it was the most awesome journey they had ever taken. | 望着眼前的美景，两人一致认为这是她们所经历过的最棒的一次旅行。 |
| 6 | net/0006.net | In addition to seeing spectacular mountain peaks and forests, one highlight of their trip was being able to see many different creatures, including deer, mountain goats, and even a grizzly bear and an eagle. | 除了欣赏壮丽的山峰和森林，旅程的亮点之一是能看到种类繁多的动物，如鹿、山羊、甚至还有一只灰熊和一只老鹰。 |

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
