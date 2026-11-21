# Confirmed protocol

Source of truth: `main` branch `recon/api.md`, `recon/wrongnote-h5.md`,
`recon/readaloud-runbook.md`, plus read-only smoke tests recorded there.

Confidence tags: **[R]** read from the decompiled client, **[S]** verified
against a live server during recon, **[T]** reproduced by a fixture in this repo.

## Transport

- POST bodies are `application/x-www-form-urlencoded`.
- Structured fields (objects/arrays) are JSON text.
- `pager###pageSize` in a field name means `pager.pageSize`.
- Wrong-note paging sends the pager in **both** body and query.

### Fixed headers on every authenticated call

```
user-agent: PC-Up366-Student 6.11.0
x-requested-with: PC
X-App-Name: student-pc
clientId: <device id>
u3r: <new UUID hex per request>
u3t: <TGT>
Authorization: ""
```

**[R]** `recon/api.md` §0.

### Auth fallback

Business responses `-21` / `-22` → `POST https://sso.up366.cn/v1/serviceTicket`
with `service=<full failing URL>` → `data.serverTicket` → replay the original
URL once with `?ticket=<st>` (or `&ticket=` when a query already exists).
**[S]** Cookies alone are sufficient on business hosts; `u3t` alone returns
`-21`.

## Endpoints in use

| Purpose | Host + path |
| --- | --- |
| Login | `sso /v2/tickets-v3` (success `code = -26`, `data.tgt`, `data.uType`) |
| Courses | `course-api /client/course/list-of-student/v2` (`status=1`, `pager.pageSize=999`) |
| Jobs | `studytask-api /client/student/course/job` (`data.allTaskList`) |
| Job history | `studytask-api /client/student/all/course/range/job` (`data[]`) |
| Job detail | `studytask-api /client/student/job/info` (`studyTaskId = jobId`) |
| Homework submit | `xot-api /client/onlinetesting/submit` (whole answer document) |
| Finished/returned | `studytask-api /client/student/all/course/finish/job/v2` |
| Finished scores | `dsb-api /client/book/finished-task/list` |
| Books (purchased) | `book-api /client/books/buy-book` |
| Course books | `course-api /client/student/course/list` (`data[].books`) |
| Book info | `book-api /client/v2/book/info` |
| Book tree | `book-api /client/books/tree` |
| Book page list | `dsb-api /client/student/chapter-task-score/list` |
| Textbook paper | `xot-api /client/textbook/paperinfo` |
| Book plan | `studytask-api /client/book/plan` |
| Answer detail v2 | `dsb-api /client/student/task/answer/detail/v2` |
| Answer detail v1 | `dsb-api /client/student/task/answer/detail` (GET) |
| Paper XML | `qlib-api /client/paper/detail?paperId=` |
| Word bucket | `words-v2-api /client/sync/teaching/bucket/detail-info` |
| Word PK submit | `wordsbtl-api /client/wordsbtl/student/submit` |
| Task score | `study-api /client/task/score/submit/v2` |
| OSS STS | `setup-api /front/aliyunoss/token/v2?organId=` |
| Server time | `setup-api /front/current-time` |

## Status fields

- Finished when `taskStatus == 1` **or** `stuStatus > 1`.
- Returned when `stuStatus == 1` and `taskStatus != 1`.
- `extParam` and `extParam.xotParam` are JSON strings that must be parsed twice.
- Word PK marker: `xotParam.pkBucketId` present, or `wordpkPlayType` key exists.

### The job list alone cannot answer "finished"

The `course/job` rows observed on 2026-11-08 carry `extParam.taskStatus == 0`
for every job and no `stuStatus` at all, so a resolver that trusts only those
fields marks everything pending or expired. The authoritative rows come from
`studytask-api /client/student/all/course/finish/job/v2`:

- `data.finishedList` and `data.returnedList`; each row is keyed by
  `studyTaskId`, which equals the job's `jobId` (never `contentId`).
- The call is **account-wide**: it ignores `courseId` and returns the same rows
  for every course, so fetch it once per refresh.
- `extParam.taskScore` is the paper's full score, not the student's score; only
  `finishedList[].score` may be shown as a result.

### Historical jobs fall out of `course/job`

`course/job` only returns live assignments; completed terms disappear from
`data.allTaskList`. Verified 2026-11-16 on course `9073115`: `allTaskList` was
empty while the account still had 142 finished rows in that course. The official
H5 "历史" tab uses a range endpoint:

- `POST studytask-api /client/student/all/course/range/job`
  `{courseId, beginTime, endTime}` with `yyyy-MM-dd` dates.
- `data` is a flat array of job rows shaped like `allTaskList` (including
  `jobName`, `beginTime`, `endTime`, `courseId`, `extParam` and `jobScore`).
- `courseId` is accepted but ignored: the call is account-wide. A single
  `2000-01-01..today` range returned 779 rows across five courses for the
  reference account.
- Assignments whose `beginTime` is still in the future are omitted, so the live
  `course/job` list is still required. Merge the two by `(courseId, jobId)`.
- The official H5 history tab sorts by `beginTime` descending.

## Books

`book-api /client/books/tree` returns three flat arrays:

- `chapters`: type 1 nodes with `id`, `parentId`, `name`.
- `pages`: type 2 nodes with `name`, `parentId` (a chapter id) and `taskId`.
- `tasks`: type 3 nodes with `id`, `parentId` (a chapter id), `taskType`.

The shelf is the union of `/client/books/buy-book` (purchased goods) and
`data[].books` from `/client/student/course/list` (class-assigned books such as
新概念英语). The two lists overlap, so entries are deduplicated by `bookId`.
`book_info` / `books/tree` take the shelf's `bookType`. The PC client
hardcodes `1` because its shelf (`buy-book`) only contains purchased books, but
class-assigned course books are `bookType=2`: asking `book_info` for one of
those as type 1 answers `code: 0` with no `data` (verified on 新概念英语2,
语法专项练习, 同步英语培优计划-同步词汇, 假期充电周), while `bookType=2` returns
the record.

`books/tree` needs the assigning `courseId` for some course books: 新概念英语1
answers zero chapters with `bookType=2` alone and 72 chapters / 144 pages when
`courseId=9222783` is included. Sending a course that does not own the book can
empty other books (高中新课标3000词 intermittently), so call without it first
and retry with it only when the answer has no content.

Type-2 course books return `chapters` and `pages` but **no `tasks`**: the
content pages (`isContent=1`, `paperId`) are the assignments, and the reader
keys `finished-task/list` rows on the page `id`. Ordinary books keep using
`tasks` (type 3) and pages only for display names.

The assignment list is `tasks`; a page only links to one through `taskId`, and
many chapters (for example 测验) have a task and no page at all. Iterating
pages misses those tasks, so the detail view builds one row per task and uses
the linked page's `name` when present. `task.id` is the id used by
`/client/book/finished-task/list`; completion is the intersection of the two id
sets. The shelf response carries no task counts and no display name for
`subjectId` (it is a numeric id), so progress can only be computed after
fetching the tree. Nested chapters are navigation only: a task belongs to its
nearest top-level ancestor.

The official book detail view does not use `books/tree`. It reads
`dsb-api /client/student/chapter-task-score/list` with
`courseId, bookId, uid`, which returns one flat array of chapters
(`nodeType = 1`) and assignments (`nodeType = 2`). `cid` is the same id
`finished-task/list` keys on (a task id for ordinary books, a content page id
for type-2 course books), and `taskType = 2` marks a vocabulary assignment
(`1` is an ordinary paper). The course is required: purchased books have none
on the shelf, but any course the student belongs to answers the same tree.

Opening one assignment is read-only and also keyed on
`(courseId, bookId, taskId, uid)`:
`/client/question/student/task/question-score/list` returns question rows
(`questionTypeNum`, `myAnswer`, `correctAnswer`, scores), and
`/client/word/task/student/word-skill-get/list` returns `{word, pass, ...}` for
a vocabulary task. In question rows, `questionTypeNum` 9/12/13 are recordings
and `myAnswer` is the student's audio URL.

## Answer detail

`data.paperInfo.paperData` (paper XML), `data.paperInfo.answerData` (correct
answers), `data.userAnswer.answerData` (student answers). `taskId` **must** be
the job's `contentId`; passing `jobId` yields empty `paperInfo`. v2 requires
`courseId` or returns `param_missing`.

## Read-aloud

Verified against two live homework submissions (2026-11-03 and 2026-11-09),
including one expired job the server still accepted.

- Question type markers: `question_type` `9` (spoken follow-read), `12`
  (spoken Q&A) and `13` (retelling) are all recordings; one recording per
  **question**, not per sentence. The client collects `urlList` entries only
  for these three types, and the book question rows report `questionTypeNum`
  9/12/13 as recordings. **[R]** `recon/readaloud-complete.md` §4.5. The
  paper's `<attachment>` holds the sentences.
- `<element type="0">` nodes are group captions (`<element_caption>`), not
  questions: they carry no `question_type` and must be ignored when deciding
  whether a paper is read-aloud-only.
- Paper source: `GET /client/paper/detail?paperId=`, `data.paperData` plus
  `data.pathPrefix`.
- OSS object name: `storage/{userType}/study/a/{yyyy/mm/dd}/{uid}/{file}`.
  The date is local, `userType` is the account's `uType` (for example `102`).
- OSS credentials: `GET /front/aliyunoss/token/v2?organId={schoolId}` returns a
  **list** of `{biz, ossBucketInfo}`; recordings use `biz = up366`.
- Upload is a signed `PUT` (Aliyun OSS V1): base64 `Content-MD5`, RFC 1123
  `Date`, and `x-oss-security-token:{token}` included in the signature. A hex
  MD5 or an unsigned security token is rejected with `403`.
- Submit branch A: `POST /client/task/score/submit/v2`, form fields
  `tasksJson`, `submitType = 8`, `ut`.
- `studyTaskId` **must equal `jobId`**; `taskId = chapterId = contentId`.
- `result` is a JSON string `{studentid, testInfo:{answerdata, markdatao}}`;
  `answerdata` has one `<element id>` per question with `<answer_url>`,
  `<answer_score>` and `<answer_recordsec>`; `markdatao` has one `<mark_data>`
  per question.
- `qstJson` is a flat **array**: one `elementType = 1` row per question and one
  `elementType = 2` row per sentence (`{questionId}-{n}`).
- Completion is the `studyTaskId` appearing in
  `/client/student/all/course/finish/job/v2`; the server may asynchronously
  re-grade the recording, so the reported score is not final.

## Course-book pages (新概念英语 and other class books)

A class-assigned course book (`bookType = 2`) has assignments that are **not
scheduled jobs**: `/client/student/course/job` contains no row for them, so a
job-id join can never resolve. The official reader still submits them through
`/client/task/score/submit/v2`, with a different row.

- Book tree: `POST book-api /client/books/tree`
  `{bookId, version:"", bookType:2, courseId}`. `data.pages[]` are the
  assignments: `id` equals the `cid` of
  `/client/student/chapter-task-score/list`, `paperId` opens the paper, and
  `isContent = 1` marks an assignable page. **[S]** 新概念英语1
  (`bookId=B8FB77D2AAAEE151EE2DA265EB9CA6D1`, `courseId=9222783`): 72 chapters,
  144 content pages, 216 chapter-task nodes; every task `cid` matches a page
  `id`.
- Paper: `POST xot-api /client/textbook/paperinfo` `{paperId}` returns
  `{paperData, answerData, pathPrefix, score, zipPath}`. `pathPrefix` is
  already absolute. This is the source `textbook.html` uses, not
  `/client/paper/detail`. **[S]** `paperId=C7C46EB9…` answered an 8-question
  paper with `score=100`.
- Submission row: the reader's `saveMarkData` passes
  `{bookId, taskId, chapterId, courseId, studyTaskId, score, percent,
  duration, userAnswerXmlStr, markDataXmlStr, submitType}` through the
  `submitAnswerData(..., "textbook", ...)` bridge, and the main process's
  textbook branch posts branch A with
  `result = {studentid, testInfo:{answerdata, markdatao}}` and `qstJson=""`.
  For a book page `studyTaskId` is empty, so the reader sets
  `submitType = studyTaskId ? 4 : 0` — **0**, not 8. `taskId` and `chapterId`
  are both the page id. **[R]** `textbook.<hash>.js`, main-window.js
  `submitAnswerData`.
- Legacy PC reader branch: extracted `main-window.js` also exposes
  `submitTaskV1`. It calls `uploadTaskScore` with `srcType = 1`, an empty
  `result`, `qstJson = ""` at the initial row boundary, and the current window's
  `StudyTaskId`. Before upload, `submitEleAttr` has already stored each
  `{pageId, elementId, elementType, addTime, order, elementAttr}` row in
  `TaskStuded.qstJson`; `elementAttr` is stored as a JSON string and the upload
  worker flattens the keyed object to an array. For `packType < 7`, this legacy
  row is the relevant reader shape; the two branches must not be conflated.
  **[R]** `extract/app/dist/main/main-window.js`.
- Completion: `/client/book/finished-task/list {bookId}` gains a row whose
  `taskId` is the page id — the same id the book detail intersects for its
  progress.
- Composite papers: `question_type = 99` is a group header, not a question.
  The reader emits it as an `<element type="1">` followed by its sub-questions
  as flat `<element type="3">` siblings (`ref_question_id` points back). It
  carries no answer and must be ignored when deciding whether a paper is
  read-aloud-only: the Lesson 1 听读 paper (70-point group + 30-point
  recording) is read-aloud-only. **[S]**

Both course-book branch shapes are observed in the official reader bundle, and
the page/paper reads are live. The one live v2 attempt from this client on
2026-11-21 used the textbook bridge (`srcType = 2`, XML `result`, empty
`qstJson`) and returned `result.code = 0` but left all eight
`question-score/list` rows at `score = 0`, `myAnswer = ""`, left the book task
node at score 0, and did not add a `finished-task/list` row. That attempt is
therefore **not** an end-to-end confirmation of the textbook payload. The
`packType < 7` legacy row is static evidence only and has not been sent; see
`to-probe.md` §12 for the exact target, observations and no-retry boundary.

## Applied writing (应用文)

- Question marker: `question_type = 14` (`qlib_qst_type_name = 应用文写作`).
  `question_type = 18` is 读后续写, a different and unsupported branch.
  **[S]** read-only on the reference account, 2026-11-16.
- The H5 homework player reads the paper through
  `/client/onlinetesting/alldata` and serialises each writing answer with the
  xot engine's `getPaperXML`: `<element id="…"><answers><![CDATA[text]]></answers></element>`
  and no `markData`. **[R]** `homework.<hash>.js` `paperSubmit`,
  `common-xot-test.<hash>.js` `getPaperXML` case 14.
- Submit: form POST `xot-api /client/onlinetesting/submit` with
  `testId` = the job's `contentId`, `paperId`, `uid`, `answerData` and
  `duration`. The H5 adds `headers[clientId]`; the Electron client queues the
  same row as `TestStuded`, where `taskId` is copied into `testId` on send.
  **[R]** `api.md` §1.14, `homework.<hash>.js`, engine `onlinetestingSubmit`.
- Of 12 sampled jobs whose title contains 应用文, 10 answered with a single
  type-14 question. One 月考 paper mixed 20 fill-in questions with one writing
  question; one 应用文范文默写 paper was type 19 (篇章默写). This client refuses
  both, because it cannot answer the non-writing part. **[S]**
- Stored answers (`answer_detail.userAnswer.answerData`) show the
  server-normalised shape
  `<answers>'<![CDATA[text]]>'</answers><pictures></pictures>`; the client sends
  the engine shape above. **[S]** read-only, 2026-11-16.
- Completion: the job appears in `/client/student/all/course/finish/job/v2`
  with its score after the server grades the essay.

## Error-notebook: correction

The H5 wrong-note page (`cuoti-stu.up366.cn`) reads through
`error-notebook-api.up366.cn/client/personalmistake/...`: `mistakeTypes`,
`overview`, `exercises`, `page`, `answerHistory`, `knowledgePoints`,
`statistics`. These are `application/json` POSTs carrying `studentId`,
`organId` and `stageId` as numbers. **[R]** H5 bundle `src/api/error-notebook-api/index.ts`.

### Draw

`POST /client/personalmistake/correction/draw` — **form-urlencoded**, not JSON.

| Field | Notes |
| --- | --- |
| `studentId` | number |
| `questionIdList` | JSON array text; omitted (or `[]`) for "by count" |
| `questionCount` | number of questions to draw |
| `groupKey` | optional overview group (by-question-type correction) |

A successful call returns `data.{paperData, answerData, pathPrefix, batchId}`.
`paperData` is the usual `<elements>` paper, `answerData` holds the correct
answers, `pathPrefix` is the media prefix. Every call creates a **new**
`batchId`; an empty `data` means there is nothing left to correct. **[S]**
live: `studentId=39370562, questionCount=5, groupKey=…` returned all four
fields with `pathPrefix=https://qlib-cdn-up366-alyun.up366.cn`.

### Status

`POST /client/personalmistake/correction/status` — **form-urlencoded**.
Fields `batchId` and `uid` (uid sent as text). Response:
`{ status, paperInfo, userAnswer, percent, submitDate }` where `status` is
`0` not submitted, `1` marking, `2` report ready. Poll until it stops being
`1`. **[S]** live: fresh batch answers `status: 0`.

### Submit

The answer page hands the payload to the Electron main process, which posts to
`https://study-api.up366.cn/client/task/score/gzip/submit` with
`Content-Type: application/json;charset=UTF-8` and `Content-Encoding: gzip`
(the body itself is gzipped):

```json
{ "tasksJson": "<JSON string of [tasksJson]>", "ut": "...", "submitType": 11 }
```

`tasksJson` holds `uid, batchId, courseId, studyTaskId, bookId, bookType,
chapterId, paperId, taskId, seconds, percent, score, testId, taskNo, qstJson,
deviceType, clientId, studyDate, wrongnoteFlag` and
`extendParams.{batchId, paperDataXml, answerDataXml, xmlResult}`. `testId`
falls back to `759E3BB039A9674D6697A88433DC5D6B`. `ut` is the task-score
formula with the literal `submitTaskToken`:

```text
r  = md5(ts)
ut = r[..10] + md5(md5(tasksJson) + "submitTaskToken-pc-987654" + r) + r[10..]
```

After the submit the page calls `POST study-api /client/job/submit` with
`jobJson = json({uid, courseId, studyTaskId})` ("错题消灭"). **[R]**
main bundle `submitTask`/`buildUT` and the correction route chunk.

**[S]** live 2026-11-09: one drawn question submitted with this payload was
accepted; `correction/status` went `1 → 2` (once straight to `2`), `percent`
reached `100`, and the overview moved `remaining -1 / corrected +1 / today +1`
on both runs. The submit response itself is `data = null`.

Answer-document shapes that the builder must handle:

- fill-blank: `<answers><answer id="1" score="1">assumptions</answer></answers>`
  — one mark per blank, type `4` (also `6`, `8`, `10`, `11`).
- choice / multi-choice: `<answers>ABC</answers>` — the parent row carries the
  answer; there are no sub-question marks.
- mark data: `<mark_data id="1"><score>..</score><result>1</result>
  <teacher_marked>0</teacher_marked><user_answer>..</user_answer></mark_data>`.

## Business codes

| Code | Meaning |
| --- | --- |
| `0` | success |
| `-13` | already submitted / attempts exhausted |
| `-21` | not logged in |
| `-22` | ST validation failed |
| `-24` | service-ticket success |
| `-26` | login success |
