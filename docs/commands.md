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
| `book_detail` | `account_id, book_id` | `BookDetail` (book + chapters + job rows) |

`books_list` is shelf metadata only: a book's total task count needs the whole
tree (megabytes for a large reader book), so it is fetched on the detail page.
`book_detail` groups each page under its top-level chapter and derives
`progress` / `total` from `finished-task/list`. Book rows are display-only:
a page's `taskId` is not a job id, so the answer route cannot open it.

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
| `wordpk_preview` | `account_id, job_id` | `WordPkPreview` (includes `confirm_token`) |
| `wordpk_submit` | `confirm_token, answers` | `WordPkResult` |

The submit call re-reads the bucket from the server, so the submitted words are
the words the server still returns, and the token is consumed before the
network call so a double click cannot submit twice.

## Read-aloud

| Command | Args | Returns |
| --- | --- | --- |
| `readaloud_get` | `account_id, job_id` | `ReadAloudTask` |
| `readaloud_upload` | `account_id, job_id, recording{question_id, path, file_name}` | `ReadAloudUpload` |
| `readaloud_submit` | `account_id, job_id, dry_run` | `ReadAloudPreview` |

Upload and submit are separate on purpose. Two limits are enforced in code, not
just in copy:

- `readaloud_upload` only **validates** the mp3 (header, size, hash). It
  returns `record_url: null`, meaning the slot is "ready" — the OSS transfer is
  not wired.
- `readaloud_submit` returns a preview by default and the guarded protocol
  layer refuses to send, because the submit branch, `mark_datao` numbering and
  `qstJson` shape are not confirmed against a real homework task.

## Wrong-notes

| Command | Args | Returns |
| --- | --- | --- |
| `wrongnotes_subjects` | `account_id` | `WrongSubject[]` |
| `wrongnotes_knowledge` | `account_id, filter{subject_id, book_id, stage_id?}` | `WrongKnowledge[]` |
| `wrongnotes_pending` | `account_id, filter{subject_id, book_id, knowledge_id, qtype_id, stage_id?}` | `WrongNote[]` |
| `wrongnotes_preview` | `account_id, question_ids` | `CleanupPreview` |
| `wrongnotes_cleanup` | `confirm_token` | `CleanupRun` |

`CleanupPreview.granularity_confirmed` decides what the run does. While it is
`false`, `wrongnotes_cleanup` reports every question as `skipped` and sends
nothing — the server's acceptance of one submission per question is unproven
(`docs/protocol/to-probe.md` §6). Once confirmed, the run is serial, reports
each question separately, and stops after three consecutive failures so a
partial batch can be resumed rather than replayed.

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
