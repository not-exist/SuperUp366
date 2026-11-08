// One function per Tauri command. Pages never build argument objects by hand.
//
// Names must match the Rust `generate_handler!` list in `src-tauri/src/lib.rs`
// exactly, including argument keys, because Tauri matches arguments by name and
// silently drops anything unexpected.

import { invokeCommand } from "./invoke";
import type { Settings, WordPkAnswer } from "./types";

export const api = {
  listAccounts: () => invokeCommand("accounts_list", {}),
  login: (username: string, password: string) =>
    invokeCommand("account_login", { username, password }),
  switchAccount: (accountId: string) =>
    invokeCommand("account_switch", { account_id: accountId }),
  logout: (accountId: string) =>
    invokeCommand("account_logout", { account_id: accountId }),

  listCourses: (accountId: string) =>
    invokeCommand("courses_list", { account_id: accountId }),
  listJobs: (accountId: string, refresh = false) =>
    invokeCommand("jobs_list", { account_id: accountId, refresh }),

  listBooks: (accountId: string) =>
    invokeCommand("books_list", { account_id: accountId }),
  getBook: (accountId: string, bookId: string) =>
    invokeCommand("book_detail", { account_id: accountId, book_id: bookId }),

  getAnswer: (accountId: string, jobId: string) =>
    invokeCommand("answer_get", { account_id: accountId, job_id: jobId }),

  getWordPk: (accountId: string, jobId: string) =>
    invokeCommand("wordpk_preview", { account_id: accountId, job_id: jobId }),
  submitWordPk: (confirmToken: string, answers: WordPkAnswer[]) =>
    invokeCommand("wordpk_submit", { confirm_token: confirmToken, answers }),

  getReadAloud: (accountId: string, jobId: string) =>
    invokeCommand("readaloud_get", { account_id: accountId, job_id: jobId }),
  uploadRecording: (
    accountId: string,
    jobId: string,
    questionId: string,
    path: string,
    fileName: string,
  ) =>
    invokeCommand("readaloud_upload", {
      account_id: accountId,
      job_id: jobId,
      recording: { question_id: questionId, path, file_name: fileName },
    }),
  submitReadAloud: (accountId: string, jobId: string, dryRun: boolean) =>
    invokeCommand("readaloud_submit", {
      account_id: accountId,
      job_id: jobId,
      dry_run: dryRun,
    }),

  listWrongSubjects: (accountId: string) =>
    invokeCommand("wrongnotes_subjects", { account_id: accountId }),
  listWrongKnowledge: (accountId: string, subjectId: string, bookId?: string) =>
    invokeCommand("wrongnotes_knowledge", {
      account_id: accountId,
      filter: { subject_id: subjectId, book_id: bookId },
    }),
  listWrongPending: (
    accountId: string,
    subjectId: string,
    bookId: string,
    knowledgeId: string,
    qtypeId: string,
  ) =>
    invokeCommand("wrongnotes_pending", {
      account_id: accountId,
      filter: {
        subject_id: subjectId,
        book_id: bookId,
        knowledge_id: knowledgeId,
        qtype_id: qtypeId,
      },
    }),
  previewWrongNoteCleanup: (accountId: string, questionIds: string[]) =>
    invokeCommand("wrongnotes_preview", { account_id: accountId, question_ids: questionIds }),
  cleanupWrongNotes: (confirmToken: string) =>
    invokeCommand("wrongnotes_cleanup", { confirm_token: confirmToken }),

  getSettings: () => invokeCommand("settings_get", {}),
  updateSettings: (settings: Settings) =>
    invokeCommand("settings_update", { settings }),
  clearCache: () => invokeCommand("cache_clear", {}),
};
