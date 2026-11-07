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

- `questions/kill` is a **read** endpoint returning `pathPrefix` / `paperData` /
  `answerData` for the kill page.
- `question/submit` is the only destructive call:

```
uid, result, startTime, endTime, seconds, rightQstIds
result = json({ "answerdata": "<student xml>", "mark_datao": "<elements>...</elements>" })
seconds = ceil((endTime - startTime) / 1000)
rightQstIds = comma-separated, or ""
```

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
