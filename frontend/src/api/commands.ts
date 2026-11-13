// One function per Tauri command. Pages never build argument objects by hand.
//
// Names must match the Rust `generate_handler!` list in `src-tauri/src/lib.rs`
// exactly, including argument keys, because Tauri matches arguments by name and
// silently drops anything unexpected.

import { invokeCommand } from "./invoke";
import type { Settings } from "./types";

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
  getBook: (
    accountId: string,
    bookId: string,
    bookType: number,
    courseId: string | null,
  ) =>
    invokeCommand("book_detail", {
      account_id: accountId,
      book_id: bookId,
      book_type: bookType,
      course_id: courseId,
    }),

  getBookTask: (
    accountId: string,
    bookId: string,
    courseId: string | null,
    taskId: string,
    wordTask: boolean,
  ) =>
    invokeCommand("book_task_get", {
      account_id: accountId,
      book_id: bookId,
      course_id: courseId,
      task_id: taskId,
      word_task: wordTask,
    }),

  getAnswer: (accountId: string, jobId: string) =>
    invokeCommand("answer_get", { account_id: accountId, job_id: jobId }),

  previewWordPk: (accountId: string, jobId: string, score: number, total: number) =>
    invokeCommand("wordpk_preview", {
      account_id: accountId,
      job_id: jobId,
      score,
      total,
    }),
  submitWordPk: (confirmToken: string) =>
    invokeCommand("wordpk_submit", { confirm_token: confirmToken }),

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
  previewReadAloud: (
    accountId: string,
    jobId: string,
    scores: { question_id: string; score: number }[],
  ) =>
    invokeCommand("readaloud_preview", {
      account_id: accountId,
      job_id: jobId,
      scores,
    }),
  submitReadAloud: (confirmToken: string) =>
    invokeCommand("readaloud_submit", { confirm_token: confirmToken }),

  getCorrectionOverview: (accountId: string) =>
    invokeCommand("correction_overview", { account_id: accountId }),
  drawCorrection: (accountId: string, count: number, groupKey?: string) =>
    invokeCommand("correction_draw", {
      account_id: accountId,
      count,
      ...(groupKey ? { group_key: groupKey } : {}),
    }),
  submitCorrection: (confirmToken: string) =>
    invokeCommand("correction_submit", { confirm_token: confirmToken }),
  getCorrectionStatus: (accountId: string, batchId: string) =>
    invokeCommand("correction_status", {
      account_id: accountId,
      batch_id: batchId,
    }),

  getSettings: () => invokeCommand("settings_get", {}),
  updateSettings: (settings: Settings) =>
    invokeCommand("settings_update", { settings }),
  clearCache: () => invokeCommand("cache_clear", {}),
};
