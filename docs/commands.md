# Command contract

The React app never talks to `up366.cn`. It calls these Tauri commands and
renders the result. The authoritative TypeScript declaration lives in
`frontend/src/api/types.ts` (`CommandMap`); this document explains the shape.

All commands take a single object argument with **snake_case** keys, and return
`Result<T, AppError>` where

```ts
type AppError = { code: string; message: string; retryable: boolean };
```

Error codes come from `up366_core::DomainError::code`:
`session_expired`, `auth_failed`, `api_error`, `duplicate_submission`,
`network_error`, `protocol_drift`, `invalid_input`, `unsupported`.

## Accounts

| Command | Args | Returns |
| --- | --- | --- |
| `accounts_list` | — | `Account[]` |
| `account_login` | `username, password` | `Account` |
| `account_switch` | `account_id` | `Account` |
| `account_logout` | `account_id` | `null` |

`account_login` never stores the password: only the resulting session (TGT,
cookies, device id) is persisted in the OS credential store.

`Account` contains only non-sensitive fields:
`account_id, username, display_name, school_name, active, session_valid,
expires_at`. Session secrets stay in the OS credential store.

## Catalog

| Command | Args | Returns |
| --- | --- | --- |
| `courses_list` | `account_id` | `Course[]` |
| `jobs_list` | `account_id, refresh?` | `JobRow[]` |

`JobRow` is the flat, cross-class list:

```ts
type JobRow = {
  job_id: string;
  course_id: string;
  course_name: string;
  class_name: string;              // label on the row, never a grouping key
  name: string;
  kind: "ordinary" | "word_pk" | "applied_writing" | "unknown";
  status: { kind: "finished" | "returned" | "expired" | "pending" | "unknown";
            score?: number | null };
  score: number | null;
  deadline: string | null;         // ISO 8601 UTC
  read_aloud: boolean;
};
```

Rows are produced by `up366_sync::merge_jobs`: courses in upstream order, then
the upstream array order inside each course. The same job under two classes
yields two rows.

## Books

| Command | Args | Returns |
| --- | --- | --- |
| `books_list` | `account_id` | `Book[]` |
| `book_detail` | `account_id, book_id, book_type, course_id` | `BookDetail` (book + chapters + assignment rows) |
| `book_task_get` | `account_id, book_id, course_id, task_id, word_task` | `BookTaskView` (words or question rows) |

`books_list` is shelf metadata only: a book's total task count needs the whole
assignment list (megabytes for a large reader book), so it is fetched on the
detail page.
The shelf is the union of purchased goods (`/client/books/buy-book`) and the
books attached to the student's courses (`/client/student/course/list`,
`data[].books`); 新概念英语 and other class-assigned books exist only in the
latter. A book in both lists is shown once. Each row carries `book_type`
(1 = purchased, 2 = class-assigned), and `book_detail` sends it back as
`bookType`: requesting a type-2 book as type 1 makes `book_info` answer
`code: 0` with no `data`. The row also carries `course_id` when a course
assigned the book. The detail endpoint requires a course, so a purchased book
without one falls back to the account's first course.
`book_detail` reads `/client/student/chapter-task-score/list`, the flat
chapter/assignment list the official reader uses. A row's `job_id` is the
node's `cid` (a task `id` for ordinary books, a content page `id` for type-2
course books), and `finished-task/list` still supplies the score.
`taskType = 2` marks a vocabulary assignment (`kind = word_pk`); recording
tasks are recognised from their title (跟读 / 朗读 / 听读 / 背诵 / 复述).
Each recording row also carries `readaloud_job_id`: when the same task appears
in the account's job list, its `contentId` equals the book `cid`, and that job
id (`studyTaskId`) is what the read-aloud page needs. The join runs against the
cached job list and refreshes the catalog once if nothing is cached yet. A
`null` `readaloud_job_id` means the task is not a scheduled job.

A class-assigned course book (`bookType = 2`) has no job row for its pages at
all, so the join is always `null`. Its recording rows also carry
`readaloud_book = true`: the page is still submittable through the official
reader's textbook branch, which uses the page/paper ids and an empty
`studyTaskId`. The UI labels those rows `含朗读 · 可提交` and opens the book
read-aloud route. Ordinary books (`bookType = 1`) keep the read-only view when
the job join is missing.

A book task id is not a job id, so it does not go through the job cache.
`book_task_get` opens one row with the same `(courseId, bookId, taskId, uid)`
the official view sends: `word_task = true` reads
`word/task/student/word-skill-get/list`, otherwise
`question/student/task/question-score/list`. Both are read-only. The page
shows words with their mastery, or question rows; read-aloud questions
(`questionTypeNum` 9/12/13) carry the student's recording in `my_answer`.

## Answers

| Command | Args | Returns |
| --- | --- | --- |
| `answer_get` | `account_id, job_id` | `AnswerDocument` |

`AnswerDocument.task_id` is the job's `contentId` — that is what the upstream
endpoint requires, not the job id. `paper_xml` is kept verbatim for the
collapsed raw view. `applied_writing` is true when the paper is a pure
`question_type = 14` paper; the answer page then links to the writing submit
route. A mixed paper is false, because the writing page refuses to submit the
non-writing part empty.

## Applied writing (应用文)

| Command | Args | Returns |
| --- | --- | --- |
| `applied_writing_get` | `account_id, job_id` | `AppliedWritingTask` (detected writing questions) |
| `applied_writing_preview` | `account_id, job_id, answers[{question_id, text}], duration_sec` | `AppliedWritingPreview` (answer XML + `confirm_token`) |
| `applied_writing_submit` | `confirm_token` | `AppliedWritingSubmit` |

`applied_writing_get` reads the job's paper and keeps only `question_type = 14`
(`应用文写作`) questions. A paper that mixes writing with other question types is
refused with `unsupported`: this client can only answer the writing part, and
submitting the rest empty would silently burn the task. The job list's
`kind = applied_writing` is a title hint (应用文 / 应用写作); the paper check is
authoritative. The job list routes to the ordinary answer page first; that page
offers a submit link only after the paper has been confirmed as pure writing.

`applied_writing_preview` requires one non-empty essay per detected question
(3000 characters excluding line breaks, the official editor's limit), builds
the H5 player's `answerData` document

```xml
<elements><element id="…"><answers><![CDATA[essay text]]></answers></element></elements>
```

and stores it in a single-use token together with the job's `contentId`,
`paperId`, uid and study seconds. `applied_writing_submit` posts the document to
`xot-api /client/onlinetesting/submit` with `testId` and `taskId` set to
`contentId`, then reads the account-wide finished list back. No score is sent:
the server grades the essay. Expired jobs are warned about, not blocked.

## Word PK

| Command | Args | Returns |
| --- | --- | --- |
| `wordpk_preview` | `account_id, job_id, score, total` | `WordPkPreview` (includes `confirm_token`) |
| `wordpk_submit` | `confirm_token` | `WordPkResult` |

`wordpk_preview` draws the round: it reads the task's `pkBucketId` bucket, keeps
the unique words that carry an `entryId` (or the older `pkid`), and picks
`total` of them at random. The first `score` drawn words are the correct ones and
the rest are wrong, because Word-PK scoring is `score = rightWordList.length`.
The request is rejected when `total > 可用单词数`, `score > total` or
`total == 0`.

The drawn round is stored in the one-shot token, so `wordpk_submit` submits
exactly the words the user saw: it never re-reads the bucket and cannot be
pointed at a different round. The token is consumed before the network call, so
a double click cannot submit twice. `duration` is the seconds between the draw
and the submit, taken from server time.

## Read-aloud

| Command | Args | Returns |
| --- | --- | --- |
| `readaloud_get` | `account_id, job_id` | `ReadAloudTask` (auto-detected slots) |
| `readaloud_upload` | `account_id, job_id, recording{question_id, path, file_name}` | `ReadAloudUpload` (real OSS URL) |
| `readaloud_preview` | `account_id, job_id, scores?[]` | `ReadAloudPreview` (row + `confirm_token`) |
| `readaloud_submit` | `confirm_token` | `ReadAloudSubmit` |
| `book_readaloud_get` | `account_id, book_id, book_type, course_id, task_id` | `ReadAloudTask` |
| `book_readaloud_upload` | `account_id, book_id, task_id, recording{question_id, path, file_name}` | `ReadAloudUpload` |
| `book_readaloud_preview` | `account_id, book_id, book_type, course_id, task_id, scores?[]` | `ReadAloudPreview` |

`readaloud_get` reads `paperId` from the cached job (falling back to
`job/info`) and parses `paperData`. Every recording question becomes one slot,
so the question/sentence counts are detected, never guessed. Recording types are
`question_type = 9` (spoken follow-read), `12` (spoken Q&A) and `13`
(retelling); the official reader submits all three through the same branch and
the book question rows report the same set as `questionTypeNum`. Group captions
(`<element type="0">`, no `question_type`) are layout, not questions. A paper
that mixes recording questions with other question types is still refused,
because their submit shape is not confirmed.

A book assignment that is also a scheduled job opens this page from the book
detail with `?back=` pointing at the book; the command arguments are unchanged
because `book_detail` already resolved the real job id.

The `book_readaloud_*` commands serve a class-book page that has no job. They
read the page's `paperId` from `/client/books/tree` (the assigning `courseId` is
required), fetch the paper from `xot-api /client/textbook/paperinfo`, and build
the same view. Their submit row mirrors the reader's textbook branch: empty
`studyTaskId`, `submitType = 0`, no `qstJson`, and `taskId = chapterId = page
id`. Only `book_type = 2` is accepted; ordinary books submit through a
different branch.

`readaloud_upload` validates the mp3, uploads it to OSS with the STS
credentials, and stores the resulting URL. Uploading again for the same
question replaces the previous file.

`readaloud_preview` requires a recording for every slot, builds the exact
`tasksJson` row (including `studyTaskId = jobId`, `qstJson` and
`answerdata/markdatao`) and returns it with a single-use token. A book-page
preview stores the book/task ids in the token instead of a job id.
`readaloud_submit` consumes the token, posts once to
`/client/task/score/submit/v2`, then reads the completion list back: the
finished-job list for a job, `/client/book/finished-task/list` for a book page.

Expired jobs are not blocked: the server accepted a late submission during
recon, so the UI only warns. The app has no vendor speech-scoring engine, so
per-question scores are user-supplied and the server may re-grade them.

## Correction (error-notebook)

| Command | Args | Returns |
| --- | --- | --- |
| `correction_overview` | `account_id` | `CorrectionOverview` (remaining / corrected / today + groups) |
| `correction_draw` | `account_id, count, group_key?` | `CorrectionDraw` (questions + one-shot `confirm_token`) |
| `correction_submit` | `confirm_token` | `CorrectionSubmit` |
| `correction_status` | `account_id, batch_id` | `CorrectionStatus` |

`correction_draw` calls `personalmistake/correction/draw` and stores the paper
in the token; it does not write. The preview lists every drawn question with
the server's standard answer, so the user confirms before anything is sent.
`correction_submit` builds one `tasksJson` object with `batchId`, the paper and
answer XML, a `mark_datao` document marking every question correct, and the
`qstJson` rows, then gzips it to `study-api/client/task/score/gzip/submit` with
`submitType=11`. The token is single-use, so a double click cannot submit the
same batch twice. `correction_status` reads the marking state afterwards
(`0` not submitted, `1` marking, `2` ready).

## Settings

| Command | Args | Returns |
| --- | --- | --- |
| `settings_get` | — | `Settings` |
| `settings_update` | `settings` | `Settings` |
| `cache_clear` | — | `null` |

## Events

| Event | Payload |
| --- | --- |
| `auth://required` | `{ account_id }` |
| `sync://progress` | `{ scope, done, total }` |
| `upload://progress` | `{ job_id, question_id, sent, total }` |
| `submission://state` | `{ run_id, question_id, state }` |

## Write safety

Every mutating command either takes an explicit `dry_run` flag or a single-use
`confirm_token` issued by a preceding `*_preview` call. A token is invalidated
on use, so a double click cannot submit twice. Partial batch failures are
persisted per item and resumed rather than replayed.
