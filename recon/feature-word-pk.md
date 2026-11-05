# 单词PK 完全解读

> 结论：单词PK = **服务端下发的 `flipbook` 资源** + 客户端内置离线播放器（`xotFullScreen`）按模板 `wordPk` 渲染。
> 全部业务逻辑在下载下来的书籍包里，客户端只负责「下载 → 开窗 → 传参 → 提交兜底」。
> 证据：`%saveDir%\flipbooks\0156577C1FC3EBED47B6F91F8CB978DD`（33MB / 94 个 `.u3enc`，已解密到 `work/flipbook-pk`）。

---

## 1. 入口与判定

H5（`student.up366.cn`，h5Ver 6.12.1）`#/course/9222783/homework/2` → 点「试题回顾」。

```js
// chunk-4a3539b4.js@51368 / chunk-70d8bd92.js@57194
o = JSON.parse(n.extParam.xotParam)
if (o.reportType == 2) { 拦截 → "《book》不适用于学生PC，请去学生移动端上学习！" }
if (o.reportType == 4) { $log({control:"任务单试题回顾按钮", event:"STUDYTASK_REVIEW"}) }
```

## 2. 从点击到渲染

```
H5 ──downloadResource({type:"flipbook", bookId, chapterId, pageId, studyTaskId,
                        testName:"10.30词汇PK/单词PK", extParams.xotParam:{...}})
   → 主进程 downloadResource           main-window.js@4009903
   → case "flipbook" → doDownloadFlipBook(bookId, bookName, e)
   → 落盘 %saveDir%\flipbooks\<bookId>\
   → case "full_screen" → MainWindow.showView("study",
        { url: …/renderer/xotFullScreen/index.html })   main-window.js@4242804
   → views.current.Param = e.data
   → 播放器 loadScript(resPath + "page1.js") → pageConfig
   → template = "wordPk"  →  加载包内 wordPk 模板 chunk
```

`page1.js` 只有 324 字节：

```js
var pageConfig = {
  template: "wordPk",
  pageElement: { element_id: "4c359f96…", element_type: 0 },
  taskObj: { task_id: "0c6399fb…", task_num: 1, taskTotal: 1 },
  dir_d: "../", pageNum: 1, pageId: "4c359f96…"
}
```

## 3. 两套维度

**赛制 `pkType`（来自 xotParam）**

| 值 | 含义 | 倒计时 | 代码 |
|---|---|---|---|
| `limit` | 限时赛，`time` 秒内答完 | 有 | `isScoring = false` |
| `score` | 计分赛，答完为止 | 无 | `isScoring = true` |

```js
t.gameType  = "limit" === t.taskInfo.pkType ? "限时赛" : "计分赛"
t.isScoring = "score" === t.taskInfo.pkType
```

**对战模型 `competitionModel`**

| 值 | 场景 | 对手数据来源 | 提交 type |
|---|---|---|---|
| `single` | 邀请好友 1V1 | `word-king/detail`（真实账号） | 1 |
| `course` | 班内排行 | **本地模拟** | 2 |
| `global` | 词王争霸 | **本地模拟** | 3 |

## 4. 关键发现：course/global 的对手是本地随机数

`js/30761301-8dca183a.js`：

```js
addRandomPersonScore: function () {
  if ("global" === this.competitionModel) {
    var e = this.groupInfo.group[1];          // 蓝方（对手）
    if (e.score !== e.rightNum) {
      if (this.isScoring) {
        Math.random() > 0.28 && (this.groupInfo.group[1].score += 1);   // 72% 概率加分
        setTimeout(t.addRandomPersonScore, 1000 * this.$store.state.randomTime);
      } else {
        setTimeout(() => { t.groupInfo.group[1].score += 1; t.addRandomPersonScore() }, e.duration / e.rightNum);
      }
    }
  }
}
```

即 **对手分数是按 `Math.random()` 本地生成的**，不是实时对战、也不是服务端推流。
`single`（邀请 1V1）才有真实对手信息（`blueName/blueScore/blueAvatar` 来自 `word-king/detail`），但也是**异步比对**，不是同步对局。

## 5. 计分

```js
score    = rightWordList.length          // 答对数
rightNum = rightWordList.length
totalNum = isScoring ? groupInfo.words.length : wordInfos.length
duration = isScoring ? (serverTime - beginTime) : ""    // 计分赛用时，取服务端时间
```

- 答错：`wrongWordList.push`，`answerStatus = 0`
- 未作答：结束时会补齐 `answerStatus = 0, answerTime = Date.now()`
- 音乐音效由 `audioContext` 本地播放（`local://` 资源）

## 6. 提交 API —— 分两条路

**A. 任务模式（`isTask = true`，从作业点进来就是这条）**

```js
r = { totalNum, rightNum, score, wordInfos: n, duration: s }
o = { courseId, studyTaskId, roleName: "Red"|"Blue", roleType: "0",
      submitJson: r, wrongLength, score }
Object(j.Z)(o)          // Z → function C → I.c.get(P.submitData, o)
```

> **提交接口：`GET https://wordsbtl-api.up366.cn/client/wordsbtl/student/submit`**
> 参数：`courseId`、`studyTaskId`、`roleName`、`roleType`、`submitJson`（JSON 字符串）、`wrongLength`、`score`

失败兜底走客户端桥 → 主进程：

```js
Object(z.f)(t.userMessage.studyTaskId, t.submitData)
  → 主进程 → POST https://study-api.up366.cn/client/task/score/submit/v2
```

**B. 词王争霸 / 活动模式（`isTask = false`）**

```js
x = { totalNum, rightNum, duration, score }
Object(z.f)("LS_WORDPKING_USERSCORE", x)     // 本地存分数
Object(z.f)("LS_WORDPKING_USERDATA", n)      // 本地存答题明细

C = { courseId,
      submitJson: JSON.stringify({ gameModel: pkType, playModel: 'score'===taskInfo.pkType?1:0,
                                   wordsModel: taskInfo.model, totalNum, rightNum, score,
                                   wordInfos, duration }),
      inviteId, type: activeType===0 ? 4 : {single:1, course:2, global:3}[competitionModel],
      worldWinFlag: c, tokenId: $store.state.token }
Object(j.ab)(C)      // ab → function A → I.c.request(P.submitDataWordPking, C)
```

> **提交接口：`POST https://wordsbtl-api.up366.cn/client/wordsbtl/student/word-king/submit`**
> 参数：`courseId`、`submitJson`、`inviteId`、`type`(1/2/3/4)、`worldWinFlag`、`tokenId`

**胜负判定（global 模式，本地算）**

```js
c = 1
if (competitionModel === "global") {
  // 分数低 / 同分但正确率低 / 同分同正确率但用时更长 → 输
  (g < d || (d===g && w/b < f/m) || (d===g && w/b===f/m && v > h)) && (c = 0)
}
```

**错误处理**

```js
code === -13 → $pkPopup({theme})          // 次数/额度用尽
2 == +result.code (word-king/submit) → 特殊业务分支（S(result)）
其余 → 弹「网络出小差」+ 保存并退出
```

## 7. wordsbtl 接口全表（22 条）

主机：`https://wordsbtl-api.up366.cn`

| 用途 | 方法 | 路径 |
|---|---|---|
| 开始对局 | — | `/client/wordsbtl/student/start` |
| 词王争霸开始（扣次数） | — | `/client/wordsbtl/student/word-king/start` |
| 提交成绩（任务） | **GET** | `/client/wordsbtl/student/submit` |
| 提交成绩（词王争霸/活动） | **POST** | `/client/wordsbtl/student/word-king/submit` |
| 练习模式提交 | — | `/client/wordsbtl/student/submit/practice` |
| 我的记录 | — | `/client/wordsbtl/student/record` |
| 记录详情 | — | `/client/wordsbtl/student/detail` |
| 词王争霸详情（邀请对象） | — | `/client/wordsbtl/student/word-king/detail` |
| 排行 | — | `/client/wordsbtl/student/rank` |
| 词王榜 | — | `/client/wordsbtl/student/word-king/rank` |
| 活动榜 | — | `/client/wordsbtl/student/word-king/activity/rank` |
| 我的活动排名 | — | `/client/wordsbtl/student/word-king/activity/user/rank` |
| 展示排名 | — | `/client/wordsbtl/student/word-king/show-rank` |
| 剩余次数 | — | `/client/wordsbtl/student/word-king/play/limit` |
| 活动剩余次数 | — | `/client/wordsbtl/student/word-king/activity/play/limit` |
| 活动加次数 | — | `/client/wordsbtl/student/word-king/activity/extra-times-update` |
| 邀请列表 | — | `/client/wordsbtl/student/word-king/invite/list` |
| 保存邀请 | — | `/client/wordsbtl/student/word-king/invite/save` |
| 更新邀请 | — | `/client/wordsbtl/student/word-king/invite/update` |
| 词表信息 | — | `/client/wordsbtl/student/words/info` |
| 完成信息 | — | `/client/wordsbtl/student/complete/info` |
| 赛季消息 | — | `/client/wordsbtl/student/word-king/season/msg` |

跨域依赖：`course-api`（课程/成员）、`words-v2-api`（词条/音频）、`studytask-api`（任务状态）、`qlib-api`（题目）、`book-api`（热书人数）、`wordnote-api`（单词练习）、`growth-api`（AI 练习）、`mall-api`（学习卡）、`ac-api`（功能授权）、`wechat-api`（分享码）、`user-api`（用户）。

## 8. 本地存储

| key | 内容 |
|---|---|
| `LS_WORDPKING_USERSCORE` | 本局 `{totalNum, rightNum, duration, score}` |
| `LS_WORDPKING_USERDATA` | 本局逐词答题明细 `wordInfos` |

## 9. 安全含义

- 包内 `page1.js` 与 57 个 chunk **无签名**，由播放器 `loadScript` + `eval(xhr.responseText)` 执行，目录在用户可写的 `C:\Up366StudentFiles\flipbooks\` → 换包即持久化代码执行。
- 提交走明文 HTTP 语义（GET + JSON 字符串），`score`/`rightNum` 完全由客户端计算，服务端若不复核即可伪造成绩。
- `tokenId: $store.state.token` 被放进请求体，与主进程 `u3t` 双轨并存。
