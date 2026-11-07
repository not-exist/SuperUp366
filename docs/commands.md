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
| `word_pk_preview` | `account_id, job_id` | `WordPkPreview` |
| `word_pk_submit` | `account_id, job_id, answers` | `SubmitResult` |

## Read-aloud

| Command | Args | Returns |
| --- | --- | --- |
| `read_aloud_get` | `account_id, job_id` | `ReadAloudTask` |
| `read_aloud_upload` | `account_id, job_id, question_id, file_name, data_base64` | `UploadResult` |
| `read_aloud_submit` | `account_id, job_id, dry_run` | `SubmitResult` |

Upload and submit are separate commands on purpose: the UI must be able to show
"uploaded" without claiming "submitted". `dry_run` defaults to true while the
submission shape is still in `docs/protocol/to-probe.md`.

## Wrong-notes

| Command | Args | Returns |
| --- | --- | --- |
| `wrong_notes_list` | `account_id, page, page_size, course_id?` | `WrongNotePage` |
| `wrong_notes_preview` | `account_id, question_ids` | `CleanupPreview` |
| `wrong_notes_cleanup` | `account_id, question_id` | `CleanupResult` |

`CleanupPreview.batch_mode` is `per_question` or `per_page`: the UI wording and
progress model depend on which granularity the server actually accepts.

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
