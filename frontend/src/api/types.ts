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

/** One question type group from the error-notebook overview. */
export interface CorrectionGroup {
  group_key: string;
  remaining_count: number;
  qtype_name: string;
  knowledge_name: string;
}

export interface CorrectionOverview {
  remaining_count: number;
  corrected_count: number;
  today_corrected_count: number;
  groups: CorrectionGroup[];
}

export interface CorrectionQuestion {
  question_id: string;
  number: string;
  question_type: string;
  type_name: string;
  score: number;
  stem: string;
  /** Correct answer, with choice options expanded for display. */
  answer: string;
}

export interface CorrectionDraw {
  batch_id: string;
  path_prefix: string;
  questions: CorrectionQuestion[];
  total_score: number;
  /** `null` when there is nothing left to correct. */
  confirm_token: string | null;
}

export interface CorrectionSubmit {
  ok: boolean;
  message: string;
  batch_id: string;
  note_message: string | null;
}

export interface CorrectionStatus {
  /** 0 not submitted, 1 marking, 2 report ready. */
  status: number;
  percent: number;
  submit_date: number;
}

export interface WordPkResult {
  ok: boolean;
  message: string;
}

/** One drawn word as the word-PK preview renders it. */
export interface WordPkWord {
  word_id: string;
  word: string;
  phonetic: string;
  translation: string;
  audio_url: string | null;
  /** Whether this word is submitted as a correct answer. */
  correct: boolean;
}

export interface WordPkPreview {
  job_id: string;
  title: string;
  bucket_id: string;
  /** Usable words the round was drawn from. */
  pool_size: number;
  /** Requested score, which is also the number of correct words. */
  score: number;
  /** Number of drawn words. */
  total: number;
  /** Drawn words in wire order: correct first, then wrong. */
  words: WordPkWord[];
  /** One-shot token for `wordpk_submit`. */
  confirm_token: string;
}

export type UploadStatus = "idle" | "validating" | "ready" | "uploading" | "uploaded" | "failed";

export interface ReadAloudSlot {
  question_id: string;
  number: number;
  /** All sentences of the question, joined for display. */
  sentence: string;
  sentences: string[];
  /** Question maximum score. */
  score: number;
  /** Reference duration in seconds. */
  duration: number;
  /** Reference audio supplied by the server, when absolute. */
  audio_url: string | null;
  /** Set once a recording has been uploaded. */
  record_url: string | null;
}

export interface ReadAloudTask {
  job_id: string;
  title: string;
  submitted: boolean;
  /** Expired jobs can still be submitted; the UI only warns. */
  expired: boolean;
  /** Detected from the paper, not guessed. */
  question_count: number;
  sentence_count: number;
  total_score: number;
  slots: ReadAloudSlot[];
}

export interface ReadAloudUpload {
  ok: boolean;
  question_id: string;
  file_name: string;
  size: number;
  record_url: string;
}

export interface ReadAloudPreviewSlot {
  question_id: string;
  number: number;
  sentence: string;
  score: number;
  record_url: string;
}

export interface ReadAloudPreview {
  job_id: string;
  title: string;
  total_score: number;
  reported_score: number;
  /** The exact row a submission would carry. */
  row: Record<string, unknown>;
  slots: ReadAloudPreviewSlot[];
  /** Single-use token for `readaloud_submit`. */
  confirm_token: string;
}

export interface ReadAloudSubmit {
  ok: boolean;
  verified: boolean;
  score: number | null;
  batch_id: string | null;
  message: string;
}

export interface Settings {
  /** Maximum number of parallel upstream requests. */
  concurrency: number;
  request_timeout_ms: number;
  refresh_interval_sec: number;
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
    args: { account_id: string; job_id: string; score: number; total: number };
    result: WordPkPreview;
  };
  wordpk_submit: {
    args: { confirm_token: string };
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
  readaloud_preview: {
    args: {
      account_id: string;
      job_id: string;
      scores?: { question_id: string; score: number }[];
    };
    result: ReadAloudPreview;
  };
  readaloud_submit: {
    args: { confirm_token: string };
    result: ReadAloudSubmit;
  };

  correction_overview: {
    args: { account_id: string };
    result: CorrectionOverview;
  };
  correction_draw: {
    args: { account_id: string; count: number; group_key?: string };
    result: CorrectionDraw;
  };
  correction_submit: {
    args: { confirm_token: string };
    result: CorrectionSubmit;
  };
  correction_status: {
    args: { account_id: string; batch_id: string };
    result: CorrectionStatus;
  };

  settings_get: { args: Record<string, never>; result: Settings };
  settings_update: { args: { settings: Settings }; result: Settings };
  cache_clear: { args: Record<string, never>; result: null };
}
