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
| Books | `book-api /client/books/buy-book` |
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
| Wrong-note reads/writes | `wrongnote-api /client/wrongnote/...` (10 endpoints) |

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

`page.taskId` is the `task.id` used by `/client/book/finished-task/list`;
completion is the intersection of the two id sets. The shelf response carries
no task counts and no display name for `subjectId` (it is a numeric id), so
progress can only be computed after fetching the tree. Nested chapters are
navigation only: a page belongs to its nearest top-level ancestor.

## Answer detail

`data.paperInfo.paperData` (paper XML), `data.paperInfo.answerData` (correct
answers), `data.userAnswer.answerData` (student answers). `taskId` **must** be
the job's `contentId`; passing `jobId` yields empty `paperInfo`. v2 requires
`courseId` or returns `param_missing`.

## Read-aloud

- Question type marker: `question_type = 9`.
- One recording per **question**, not per sentence.
- Audio is uploaded to OSS first; the resulting URL/path is written into
  `record_url` and `objectIds`, then the score payload is submitted. Audio is
  never a direct field of the score endpoint.
- OSS object name: `storage{userType}/study/a/{yyyy/mm/dd}/{uid}/{file}`.

## Wrong-answer book

- `subject/list` returns an array; `subjectId` and `stageId` are **numbers**
  (for example `303` / `3`), not strings.
- `knowledge/list/v3` returns `data.{list,pager}` and **requires** `stageId`,
  `subjectId`, `bookId`, `startTime`, `endTime`, `orderType`, `orderField`
  and the pager; without the stage or the time range it answers `-1
  param_missing`. Its columns are spelt `qTypeId` / `qTypeName`.
- `student/question/show` requires `stageId`, `subjectId`, `bookId` (`"0"` means
  all books; an empty string is rejected), `qtypeId`, `knowledgeId`, the time
  range and the pager. It returns `paperData` / `answerData` /
  `userAnswerData` plus a `list` of rows keyed by `questionId`; the stem and
  answers are only in the XML, not in the row.
- `questions/kill` is a **read** endpoint returning `pathPrefix` / `paperData` /
  `answerData` for the kill page.
- `question/submit` is the only destructive call:

```
uid, result, startTime, endTime, seconds, rightQstIds
result = json({ "answerdata": "<student xml>", "mark_datao": "<elements>...</elements>" })
seconds = ceil((endTime - startTime) / 1000)
rightQstIds = comma-separated, or ""
```

- One question per call is accepted; the official client's cleanup submits the
  page in a single call, but per-question submissions work the same way.
- `answerdata` is an `<elements>` document with one `<element id="...">` per
  answered question, copied from the standard `answerData` shape.
- `mark_datao` is an `<elements>` document: one `<element id="...">` holding
  `<mark_data><score>..</score><result>1</result></mark_data>`.

- Auth: cookies or a one-time `?ticket=`; `u3t` alone returns `-21`.

## Business codes

| Code | Meaning |
| --- | --- |
| `0` | success |
| `-13` | already submitted / attempts exhausted |
| `-21` | not logged in |
| `-22` | ST validation failed |
| `-24` | service-ticket success |
| `-26` | login success |
