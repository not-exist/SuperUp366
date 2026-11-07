// Wire-format DTOs exchanged with the Rust side.
//
// Field names are snake_case on purpose: they mirror the serde output of the
// `up366-*` crates, so a command can return a domain struct unchanged.
// Every timestamp is an ISO-8601 string in UTC.

export type JobKind = "ordinary" | "word_pk" | "unknown";

/** Coarse status resolved by `up366_core::resolve_status`. */
export type JobStatusKind =
  | "finished"
  | "returned"
  | "expired"
  | "pending"
  | "unknown";

export interface JobStatus {
  kind: JobStatusKind;
  /** Present for `finished` when the task was graded. */
  score?: number | null;
}

export interface Account {
  account_id: string;
  /** Login name, shown again when the user has to re-authenticate. */
  username: string;
  display_name: string;
  school_name: string;
  active: boolean;
  session_valid: boolean;
  expires_at: string | null;
}

export interface Course {
  course_id: string;
  name: string;
  class_name: string;
  teacher_name: string | null;
}

/**
 * One row of the flat, cross-class job list.
 *
 * The list is never grouped by class: `class_name` is a label on the row, and
 * the same job in two classes stays two rows.
 */
export interface JobRow {
  job_id: string;
  course_id: string;
  course_name: string;
  class_name: string;
  name: string;
  kind: JobKind;
  status: JobStatus;
  score: number | null;
  /** Submission deadline; `null` when the task has none. */
  deadline: string | null;
  /** Whether the paper contains read-aloud questions. */
  read_aloud: boolean;
}

export type BookStatus = "active" | "finished";

export interface Book {
  book_id: string;
  name: string;
  subject: string;
  cover_url: string | null;
  progress: number;
  total: number;
  status: BookStatus;
}

export interface BookJob {
  job_id: string;
  name: string;
  kind: JobKind;
  status: JobStatus;
  score: number | null;
  deadline: string | null;
  read_aloud: boolean;
}

export interface BookChapter {
  chapter_id: string;
  title: string;
  jobs: BookJob[];
}

export interface BookDetail extends Book {
  teacher_name: string | null;
  chapters: BookChapter[];
}

export type AnswerStatus = "right" | "wrong" | "partial" | "unanswered";

export interface AnswerQuestion {
  question_id: string;
  number: number;
  /** Upstream question type marker, e.g. `9` for read-aloud. */
  type: string;
  stem: string;
  student_answer: string;
  correct_answer: string;
  score: number | null;
  max_score: number;
  status: AnswerStatus;
}

export interface AnswerDocument {
  job_id: string;
  /** `contentId`, which is what the answer endpoint expects, not the job id. */
  task_id: string;
  title: string;
  score: number | null;
  max_score: number;
  paper_xml: string;
  questions: AnswerQuestion[];
}

export interface WrongNote {
  question_id: string;
  job_id: string;
  course_id: string;
  class_name: string;
  title: string;
  question_type: string;
  stem: string;
  correct_answer: string;
  student_answer: string;
  added_at: string;
}

export interface WrongNotePage {
  items: WrongNote[];
  total: number;
  page: number;
  page_size: number;
}

export interface CleanupPreviewItem {
  question_id: string;
  title: string;
}

export interface CleanupPreview {
  total: number;
  estimated_seconds: number;
  /** Server-side cleanup granularity. Drives the progress wording. */
  batch_mode: "per_question" | "per_page";
  items: CleanupPreviewItem[];
}

export interface CleanupResult {
  ok: boolean;
  message: string;
  remaining: number;
}

export interface WordPkWord {
  word_id: string;
  word: string;
  phonetic: string;
  translation: string;
  audio_url: string | null;
}

export interface WordPkPreview {
  job_id: string;
  title: string;
  bucket_id: string;
  my_score: number | null;
  opponent_score: number | null;
  words: WordPkWord[];
}

export interface WordPkAnswer {
  word_id: string;
  known: boolean;
}

export type UploadStatus = "idle" | "uploading" | "uploaded" | "failed";

export interface ReadAloudQuestion {
  question_id: string;
  number: number;
  prompt: string;
  sentence: string;
  /** Reference audio supplied by the server, if any. */
  audio_url: string | null;
  /** Filled once a recording has been uploaded. */
  record_url: string | null;
}

export interface ReadAloudTask {
  job_id: string;
  title: string;
  submitted: boolean;
  questions: ReadAloudQuestion[];
}

export interface SubmitResult {
  ok: boolean;
  message: string;
  score?: number | null;
}

export interface UploadResult {
  ok: boolean;
  record_url: string;
  object_ids: string[];
}

export interface Settings {
  /** Maximum number of parallel upstream requests. */
  concurrency: number;
  request_timeout_ms: number;
  refresh_interval_sec: number;
  /**
   * Protocol note: read-aloud submission shape is not confirmed yet, so the
   * app stops at upload verification and a dry run.
   */
  read_aloud_dry_run: boolean;
  keep_audio_files: boolean;
}

/**
 * The command contract with the Tauri layer.
 *
 * Rust commands are registered with these exact names and argument keys; each
 * returns the `result` type on success and `{ code, message, retryable }` on
 * failure (see `up366_core::DomainError`).
 */
export interface CommandMap {
  accounts_list: { args: Record<string, never>; result: Account[] };
  account_login: {
    args: { username: string; password: string };
    result: Account;
  };
  account_switch: { args: { account_id: string }; result: Account };
  account_logout: { args: { account_id: string }; result: null };

  jobs_list: {
    args: { account_id: string; refresh?: boolean };
    result: JobRow[];
  };
  courses_list: { args: { account_id: string }; result: Course[] };

  books_list: { args: { account_id: string }; result: Book[] };
  book_detail: {
    args: { account_id: string; book_id: string };
    result: BookDetail;
  };

  answer_get: {
    args: { account_id: string; job_id: string };
    result: AnswerDocument;
  };

  word_pk_preview: {
    args: { account_id: string; job_id: string };
    result: WordPkPreview;
  };
  word_pk_submit: {
    args: { account_id: string; job_id: string; answers: WordPkAnswer[] };
    result: SubmitResult;
  };

  read_aloud_get: {
    args: { account_id: string; job_id: string };
    result: ReadAloudTask;
  };
  read_aloud_upload: {
    args: {
      account_id: string;
      job_id: string;
      question_id: string;
      file_name: string;
      data_base64: string;
    };
    result: UploadResult;
  };
  read_aloud_submit: {
    args: { account_id: string; job_id: string; dry_run: boolean };
    result: SubmitResult;
  };

  wrong_notes_list: {
    args: {
      account_id: string;
      page: number;
      page_size: number;
      course_id?: string;
    };
    result: WrongNotePage;
  };
  wrong_notes_preview: {
    args: { account_id: string; question_ids: string[] };
    result: CleanupPreview;
  };
  wrong_notes_cleanup: {
    args: { account_id: string; question_id: string };
    result: CleanupResult;
  };

  settings_get: { args: Record<string, never>; result: Settings };
  settings_update: { args: { settings: Settings }; result: Settings };
  cache_clear: { args: Record<string, never>; result: null };
}
