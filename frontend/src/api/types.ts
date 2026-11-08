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

/** One shelf entry. No progress here: that needs the whole book tree. */
export interface Book {
  book_id: string;
  name: string;
  subject: string;
  cover_url: string | null;
  /** `packType == 9` books use the newer reader. */
  is_2025_reader: boolean;
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
  /** Pages whose task appears in the finished-task list. */
  progress: number;
  /** Pages that carry a task id. */
  total: number;
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

export interface CleanupPreviewItem {
  question_id: string;
  title: string;
}

export interface CleanupPreview {
  items: CleanupPreviewItem[];
  total: number;
  /**
   * False until per-question submission is confirmed against a real account.
   * While false the run refuses to send, so the UI must say so instead of
   * showing a submit button that silently does nothing.
   */
  granularity_confirmed: boolean;
  /** One-shot token for `wrongnotes_cleanup`. */
  confirm_token: string;
}

/** One question's outcome inside a cleanup run. */
export interface CleanupItemResult {
  question_id: string;
  /** `cleared`, `failed`, or `skipped`. */
  state: string;
  message: string;
}

export interface CleanupRun {
  ok: boolean;
  message: string;
  items: CleanupItemResult[];
  /** How many questions were actually cleared. */
  cleared: number;
}

/** One subject row, used to fill the picker. */
export interface WrongSubject {
  subject_id: string;
  subject_name: string;
  wrong_count: number;
  killed_count: number;
}

/** One knowledge point. */
export interface WrongKnowledge {
  knowledge_id: string;
  knowledge_name: string;
  qtype_id: string;
  qtype_name: string;
  wrong_count: number;
}

export interface WordPkResult {
  ok: boolean;
  message: string;
}

/** One word as the word-PK picker renders it. */
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
  /** One-shot token for `wordpk_submit`. */
  confirm_token: string;
}

export interface WordPkAnswer {
  word_id: string;
  known: boolean;
}

export type UploadStatus = "idle" | "validating" | "ready" | "uploading" | "uploaded" | "failed";

export interface ReadAloudSlot {
  question_id: string;
  number: number;
  prompt: string;
  sentence: string;
  /** Reference audio supplied by the server, if any. */
  audio_url: string | null;
  /** Set once a recording has been uploaded; `null` means "ready" only. */
  record_url: string | null;
}

export interface ReadAloudTask {
  job_id: string;
  title: string;
  submitted: boolean;
  slots: ReadAloudSlot[];
  /** Always false until the submission payload is confirmed. */
  submission_enabled: boolean;
}

export interface ReadAloudUpload {
  ok: boolean;
  question_id: string;
  file_name: string;
  size: number;
  /** `null` until the OSS transfer is wired: the slot is "ready", not uploaded. */
  record_url: string | null;
}

export interface ReadAloudPreview {
  job_id: string;
  title: string;
  /** The exact payload a submission would carry. */
  payload: Record<string, unknown>;
  /** True when nothing was sent. */
  dry_run: boolean;
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

  wordpk_preview: {
    args: { account_id: string; job_id: string };
    result: WordPkPreview;
  };
  wordpk_submit: {
    args: { confirm_token: string; answers: WordPkAnswer[] };
    result: WordPkResult;
  };

  readaloud_get: {
    args: { account_id: string; job_id: string };
    result: ReadAloudTask;
  };
  readaloud_upload: {
    args: {
      account_id: string;
      job_id: string;
      recording: { question_id: string; path: string; file_name: string };
    };
    result: ReadAloudUpload;
  };
  readaloud_submit: {
    args: { account_id: string; job_id: string; dry_run: boolean };
    result: ReadAloudPreview;
  };

  wrongnotes_subjects: { args: { account_id: string }; result: WrongSubject[] };
  wrongnotes_knowledge: {
    args: {
      account_id: string;
      filter: {
        subject_id?: string;
        book_id?: string;
        knowledge_id?: string;
        qtype_id?: string;
        stage_id?: string;
      };
    };
    result: WrongKnowledge[];
  };
  wrongnotes_pending: {
    args: {
      account_id: string;
      filter: {
        subject_id?: string;
        book_id?: string;
        knowledge_id?: string;
        qtype_id?: string;
        stage_id?: string;
      };
    };
    result: WrongNote[];
  };
  wrongnotes_preview: {
    args: { account_id: string; question_ids: string[] };
    result: CleanupPreview;
  };
  wrongnotes_cleanup: { args: { confirm_token: string }; result: CleanupRun };

  settings_get: { args: Record<string, never>; result: Settings };
  settings_update: { args: { settings: Settings }; result: Settings };
  cache_clear: { args: Record<string, never>; result: null };
}
