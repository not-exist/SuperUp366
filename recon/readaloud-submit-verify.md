# 逐句跟读 真实提交记录（2026-11-03）

> 目标作业：`Unit 4 JOURNEY ACROSS A VAST LAND / 课文1 / 逐句跟读`（变体 A，截止 2026-11-10）
> 结果：**提交成功，作业 `taskStatus` 0→1，客户端显示已完成**
>
> ⚠️ 本文 §1 / §2 / §5 / §7 已于 2026-11-03 晚更正 —— 首次提交用了自造的 `studyTaskId`，
> 导致"服务端有记录但全部客户端显示未完成"。更正内容见 §8，操作手册见 `recon/readaloud-runbook.md`。

---

## 1. 结论

| 项 | 值 |
|---|---|
| 接口 | `POST https://study-api.up366.cn/client/task/score/submit/v2`（分支 A） |
| 业务返回 | `{"result":{"code":0,"msg":"success"}}` |
| 服务端记录 | `taskId=3D1B42C891F7080656123366A77DD5A7`，`seconds=693`，`batchId=d90f534c804dab721adbe4404a04a92f`，`studyDate=1793714405924` |
| 上报分数 | 每题 8 分 × 3 题 = 24 / 30 |
| 落库分数 | 首次读到 24；稍后同一条记录变成 **8.2**（`firstScore=8.2`，`percent` 仍 100）—— 疑似服务端异步复评，见 §4.1 |
| 作业状态 | `job/info.taskStatus: 0 → 1`，`submitTime` 非 0，`stuStatus: 2` |
| 音频 | 3 段，各落在 OSS `storage/102/study/a/2026/11/03/39370562/ra*.mp3` |

**"服务端不复核分数"这条早期结论作废。** 见 `recon/readaloud-runbook.md` §4.1。

---

## 2. 本次作业的全部 id（实测来源 `POST studytask-api/client/student/job/info`）

```jsonc
{
  "taskId":    "3D1B42C891F7080656123366A77DD5A7",   // = chapterId = pageId = contentId
  "chapterId": "3D1B42C891F7080656123366A77DD5A7",
  "paperId":   "1D758BD86E8BE3AF0797789D68EF531B",
  "bookId":    "9962655BB56EE1AD4128EFE0251D16B7",
  "courseId":  9222783,
  "taskScore": 30,
  "jobId":     "251C2193728510A14A02552B4ED44033",
  "studyTaskPid": "4750B87661862EEE789DEADB75E4A01A"
}
```

`studyTaskId` **= 作业列表里的 `jobId`**（本次 `251C2193728510A14A02552B4ED44033`）。
它不在 `finished-task` 类接口里显式出现，但 `job/info` 返回的 `jobId` 就是它 —— 首次提交自造了一个
随机 32 hex，服务端仍写入了成绩记录，但**无法关联到 job**，所以 `taskStatus` 恒为 0。

---

## 3. 提交行（`TaskStuded`）关键字段

| 字段 | 值 | 说明 |
|---|---|---|
| `taskId` | `3D1B...` | 内容 id |
| `score` / `percent` | 24 / 80 | percent 服务端按 `score/maxScore` 重算成 100 |
| `seconds` | 693 | 三段真实音频时长之和 |
| `srcType` | 2 | 课本分支（`submitAnswerData("textbook")` 同款） |
| `result` | `JSON.stringify({studentid, testInfo:{answerdata, markdatao}})` | 3355 字符，两份 XML |
| `qstJson` | 对象（键 `${elementId}_${order}`）→ 提交前拍平成数组 | 18 条（3 题 × (1+4/5/6)） |
| `extendParams` | `{"batchId":"…"}` | 字符串，客户端会 parse 成对象再发 |
| `submitType` | 8 | 与本地真实样本一致 |
| `uploaded` | 提交前 0 → 成功后置 2 | 防止客户端重放撞 -13 |

**XML 形状**（`result.testInfo`）：

```xml
<!-- answerdata：每题一个 element，音频 URL 在这里 -->
<elements><element id="…"><answers>
  <answer_url>https://up366-bucket1.oss-cn-beijing.aliyuncs.com/storage/102/study/a/2026/11/03/39370562/ra…mp3</answer_url>
  <answer_score>8</answer_score><answer_rejectscore>0</answer_rejectscore>
  <answer_text><![CDATA[整题原文]]></answer_text>
  <answer_recordmin><![CDATA[0]]></answer_recordmin><answer_recordsec><![CDATA[82]]></answer_recordsec>
</answers></element>…</elements>

<!-- markdatao：每题一个 mark_data -->
<elements><element id="…"><mark_data><score>8</score><result>3</result></mark_data></element>…</elements>
```

`result` 语义：`1` 满分 / `2` 零分 / `3` 部分分（客户端 `getResultByEngineScore`）。8/10 → `3`。

---

## 4. 音频上传

- 用客户端自己的 STS：`GET /front/aliyunoss/token/v2?organId=3105` → `up366-bucket1`。
- 上传走客户端 `AppInfo.uploadFile(localPath, objectName)`，**objectName 由我们指定**：
  `storage/{userType}/study/a/{yyyy/mm/dd}/{uid}/{name}.mp3`（`userType=102`，`uid=39370562`）。
- 验证：`curl` 直链返回 `HTTP 200`，字节数与源文件一致。
- `record_url` 只有**题级一个**（每段音频对应一题），句级子项复用同值。

---

## 5. 风险与边界（实测确认）

1. **判重键 = `(uid, studyTaskId)`**，重复提交返回 `-13 重复作答`。
   **不要自造 `studyTaskId`** —— 自造值能写入 `finished-task`，但 job 仍是未提交（见 §8）。
2. **分数不可控**：上报 24，落库最终为 8.2（同一 `batchId`，不是新记录）。疑似服务端
   对口语题异步复评，用音频重算并覆盖 `score/maxScore`。见 `recon/readaloud-runbook.md` §4.1。
3. **音频进学校 OSS，老师侧可见**：`<answer_url>` 是可直链访问的公开 URL。
4. **本地库要同步置 `uploaded=2`**：否则客户端下次启动会重放该行（撞 `-13 重复作答`）。
5. **本地簿记三表**（`taskscore` / `studytaskrecord` / `taskstuded`）决定课本页是否显示完成，
   服务端 `taskStatus` 决定作业列表是否显示完成，两者都要顾。

---

## 6. 复现

```bash
# 1) 查作业 id（需客户端已登录）
python tools/readaloud_job.py --login <账号> --password <口令> --client-id <MachineCode> --course-id 9222783

# 2) 上传 + 构造行（dry-run 先看）
python tools/readaloud_submit.py --mp3 q1=1.mp3,q2=2.mp3,q3=3.mp3 --scores 8,8,8

# 3) 提交
python tools/readaloud_submit.py --mp3 q1=1.mp3,q2=2.mp3,q3=3.mp3 --scores 8,8,8 --send
python tools/readaloud_post.py --row work/readaloud-prep/row.json --send

# 4) 核对服务端
#    POST dsb-api /client/book/finished-task/list  {bookId}  → 按 taskId 查 score
```

> 注：`readaloud_submit.py --send` 会在本地库里插一行 `uploaded=0`；提交成功后需手动置
> `uploaded=2`（或用 `readaloud_post.py` 的响应判断后处理），避免客户端重放。

---

## 7. 为什么客户端"不显示完成"（已定位）

**根因：`studyTaskId` 不是这条作业的真实 id。**

客户端判"完成"看的是服务端 job 的 `taskStatus`（不是 `finished-task/list` 有没有记录）：

```js
// 作业列表 H5：t.studyTaskId === e.jobId 才算完成
finished: this.taskScore ? 1 : 0
```

首次提交用自造 `studyTaskId=40c12a92…` 的结果：

| 检查点 | 结果 |
|---|---|
| `finished-task/list` 有 `taskId=3D1B…` | ✅ 有 |
| `teacherScanStudyTaskIds` 含自造 id | ✅ 含（199→200） |
| `job/info.taskStatus` | ❌ **0（未提交）** |
| 其他客户端 | ❌ 同样未完成 |

→ 服务端把成绩存下来了，但关联不到 job 实例，所以**任何**客户端都不会显示完成。

### 正确做法

`studyTaskId` 必须取 `job/info` 返回的 `jobId`。实测验证：
样本 2 的 `jobId` 与其真实报文里的 `studyTaskId` **完全相同**（`4578C972…`）。

用 `jobId=251C2193728510A14A02552B4ED44033` 重新提交后：

| 检查点 | 结果 |
|---|---|
| `job/info.taskStatus` | ✅ **1** |
| `submitTime` | ✅ `1793714840149` |
| `stuStatus` | ✅ **2** |
| 客户端 | ✅ 显示已完成 |

### 本地簿记仍需补（但**不是**根因）

`initChapter` 里 `taskScore` = 本地 `taskscore` ∪ 服务端 `finished-task`（本地优先）。
绕过客户端直发 POST 不会写本地表，所以课本页仍可能不刷新。补三处：

| 表 | 内容 |
|---|---|
| `taskscore` | `bookId, taskId=3D1B…, type=2, pid=3D1B…, study=2, score=24, weight=0, percent=100, studyDate` |
| `studytaskrecord` | `taskId=3D1B…, studyTaskId=251C21…, score=24, studyDate` |
| `taskstuded` | 该行 `uploaded=2`（防止重放撞 -13） |

然后重启客户端（`initChapter` 只在加载课本时跑）。

---

## 8. 更正记录（2026-11-03 晚）

| 早期结论 | 实际 |
|---|---|
| "`studyTaskId` 可自选，服务端只当判重键" | ❌ **必须 = `jobId`**，否则 job 永远未提交 |
| "服务端早就写进去了，缺的是本地簿记" | ❌ 服务端 `finished-task` 有记录 ≠ job 已提交；`taskStatus` 才是判据 |
| "`taskStatus` 不是完成标记" | ❌ 它就是完成标记（0 未交 / 1 已交）；当时误判是因为拿它比了另一批旧任务 |
| "服务端不复核分数，上报多少落多少" | ❌ 本次记录最终被改成 8.2（同一 batchId 就地改写），疑似异步复评，未定位 |
