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
| Job detail | `studytask-api /client/student/job/info` (`studyTaskId = jobId`) |
| Finished/returned | `studytask-api /client/student/all/course/finish/job/v2` |
| Finished scores | `dsb-api /client/book/finished-task/list` |
| Books (purchased) | `book-api /client/books/buy-book` |
| Course books | `course-api /client/student/course/list` (`data[].books`) |
| Book info | `book-api /client/v2/book/info` |
| Book tree | `book-api /client/books/tree` |
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

## Books

`book-api /client/books/tree` returns three flat arrays:

- `chapters`: type 1 nodes with `id`, `parentId`, `name`.
- `pages`: type 2 nodes with `name`, `parentId` (a chapter id) and `taskId`.
- `tasks`: type 3 nodes with `id`, `parentId` (a chapter id), `taskType`.

The shelf is the union of `/client/books/buy-book` (purchased goods) and
`data[].books` from `/client/student/course/list` (class-assigned books such as
新概念英语). The two lists overlap, so entries are deduplicated by `bookId`.
`book_info` / `books/tree` still take `bookType=1`, matching the PC client,
which hardcodes that value; for a `bookType=2` course book the tree is empty.

The assignment list is `tasks`; a page only links to one through `taskId`, and
many chapters (for example 测验) have a task and no page at all. Iterating
pages misses those tasks, so the detail view builds one row per task and uses
the linked page's `name` when present. `task.id` is the id used by
`/client/book/finished-task/list`; completion is the intersection of the two id
sets. The shelf response carries no task counts and no display name for
`subjectId` (it is a numeric id), so progress can only be computed after
fetching the tree. Nested chapters are navigation only: a task belongs to its
nearest top-level ancestor.

## Answer detail

`data.paperInfo.paperData` (paper XML), `data.paperInfo.answerData` (correct
answers), `data.userAnswer.answerData` (student answers). `taskId` **must** be
the job's `contentId`; passing `jobId` yields empty `paperInfo`. v2 requires
`courseId` or returns `param_missing`.

## Read-aloud

Verified against two live homework submissions (2026-11-03 and 2026-11-09),
including one expired job the server still accepted.

- Question type marker: `question_type = 9`; one recording per **question**,
  not per sentence. The paper's `<attachment>` holds the sentences.
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
