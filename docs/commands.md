# Command contract

The React app never talks to `up366.cn`. It calls these Tauri commands and
renders the result. All commands return `Result<T, AppError>` where
`AppError = { code: string, message: string, retryable: boolean }`.

## Accounts

| Command | Args | Returns |
| --- | --- | --- |
| `accounts_list` | — | `Account[]` |
| `login` | `username, password` | `Account` |
| `logout` | `account_id` | `void` |
| `switch_account` | `account_id` | `Account` |
| `accounts_remove` | `account_id` | `void` |
| `session_status` | `account_id` | `{ state: "valid" \| "needs_reauth" }` |

`Account` carries only non-sensitive fields: `id, username, display_name, uid,
school_id, user_type, status, last_login_at`.

## Catalog

| Command | Args | Returns |
| --- | --- | --- |
| `courses_list` | `refresh: bool` | `Course[]` |
| `jobs_merged` | `refresh: bool, filter?: JobFilter` | `{ rows: JobRow[], fetched_at, stale }` |
| `job_detail` | `job_id` | `JobDetail` |

`JobRow` is flat across classes:

```ts
type JobRow = {
  job_id: string;
  name: string;
  course_id: string;
  class_name: string;      // small label next to the name
  kind: "ordinary" | "word_pk" | "unknown";
  state: "finished" | "returned" | "expired" | "pending" | "unknown";
  score?: number | null;
  max_score?: number | null;
  deadline?: string | null; // ISO 8601
  has_readaloud?: boolean;
};
```

## Books

| Command | Args | Returns |
| --- | --- | --- |
| `books_list` | `refresh: bool` | `Book[]` |
| `book_tree` | `book_id` | `BookTree` |
| `book_tasks` | `book_id` | `BookTask[]` (same shape as `JobRow`) |

## Answers

| Command | Args | Returns |
| --- | --- | --- |
| `answer_detail` | `job_id` | `AnswerDocument` |

```ts
type AnswerDocument = {
  job_id: string;
  questions: Question[];   // id, no, stem, correct, user_answer, score
  raw_paper_xml?: string;  // collapsed in the UI
  drift?: string;          // set when parsing failed
};
```

## Word PK

| Command | Args | Returns |
| --- | --- | --- |
| `wordpk_words` | `job_id` | `Word[]` |
| `wordpk_prepare` | `job_id, score, total` | `{ preview: WordPkPreview, confirm_token }` |
| `wordpk_submit` | `confirm_token` | `SubmissionResult` |

## Read-aloud

| Command | Args | Returns |
| --- | --- | --- |
| `readaloud_task` | `job_id` | `ReadAloudTask` (slots per question) |
| `import_audio` | `job_id, question_id, path` | `AudioAsset` |
| `upload_audio` | `job_id, confirm_token` | `UploadResult` |
| `readaloud_prepare_submit` | `job_id, scores?` | `{ preview, confirm_token }` |
| `readaloud_submit` | `confirm_token` | `SubmissionResult` |

`upload_audio` and `readaloud_submit` are separate on purpose: the UI must be
able to show "uploaded" without claiming "submitted".

## Wrong-notes

| Command | Args | Returns |
| --- | --- | --- |
| `wrongnote_subjects` | — | `WrongSubject[]` |
| `wrongnote_knowledge` | `subject_id, book_id, page` | `WrongNotePage` |
| `wrongnote_pending` | `filter` | `WrongQuestion[]` |
| `wrongnote_answers` | `question_ids` | `WrongAnswerBundle` |
| `wrongnote_prepare_clean` | `question_ids` | `{ items, confirm_token }` |
| `wrongnote_execute_clean` | `confirm_token` | `RunHandle` |
| `wrongnote_cancel_clean` | `run_id` | `void` |
| `wrongnote_run_status` | `run_id` | `CleanRun` |

## Events

| Event | Payload |
| --- | --- |
| `auth://required` | `{ account_id }` |
| `sync://progress` | `{ scope, done, total }` |
| `upload://progress` | `{ job_id, question_id, sent, total }` |
| `submission://state` | `{ run_id, question_id, state }` |

## Write rules

`prepare_*` returns a single-use `confirm_token` with a short TTL. `execute`
consumes it and fails with `invalid_input` if it is missing, expired or already
used. This prevents double submissions from double clicks or stale UI state.
