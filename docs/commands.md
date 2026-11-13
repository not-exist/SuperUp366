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
  kind: "ordinary" | "word_pk" | "unknown";
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
| `book_detail` | `account_id, book_id, book_type, course_id` | `BookDetail` (book + chapters + job rows) |

`books_list` is shelf metadata only: a book's total task count needs the whole
tree (megabytes for a large reader book), so it is fetched on the detail page.
The shelf is the union of purchased goods (`/client/books/buy-book`) and the
books attached to the student's courses (`/client/student/course/list`,
`data[].books`); 新概念英语 and other class-assigned books exist only in the
latter. A book in both lists is shown once. Each row carries `book_type`
(1 = purchased, 2 = class-assigned), and `book_detail` sends it back as
`bookType`: requesting a type-2 book as type 1 makes `book_info` answer
`code: 0` with no `data`. The row also carries `course_id` when a course
assigned the book; `books/tree` returns 新概念英语1's chapters only when the
request includes it, so the detail command asks without the course first and
retries with it when that answer has no content.
`book_detail` builds rows from the tree's `tasks` array (type 3), using a page
(type 2) only for the display name and falling back to the owning chapter's
title when a task has no page. Type-2 content books carry no `tasks`: there the
content pages (`isContent=1` / `paperId`) are the rows, keyed by page id. `progress` / `total` come from
`finished-task/list`. Book rows are display-only: a book task id is not a job
id, so the answer route cannot open it.

## Answers

| Command | Args | Returns |
| --- | --- | --- |
| `answer_get` | `account_id, job_id` | `AnswerDocument` |

`AnswerDocument.task_id` is the job's `contentId` — that is what the upstream
endpoint requires, not the job id. `paper_xml` is kept verbatim for the
collapsed raw view.

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

`readaloud_get` reads `paperId` from the cached job (falling back to
`job/info`) and parses `paperData`. Every `question_type = 9` element becomes
one slot, so the question/sentence counts are detected, never guessed. A paper
that mixes read-aloud with other question types is refused.

`readaloud_upload` validates the mp3, uploads it to OSS with the STS
credentials, and stores the resulting URL. Uploading again for the same
question replaces the previous file.

`readaloud_preview` requires a recording for every slot, builds the exact
`tasksJson` row (including `studyTaskId = jobId`, `qstJson` and
`answerdata/markdatao`) and returns it with a single-use token.
`readaloud_submit` consumes the token, posts once to
`/client/task/score/submit/v2`, then reads the finished-job list back.

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
