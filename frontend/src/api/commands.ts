// One function per Tauri command. Pages never build argument objects by hand.

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
    invokeCommand("word_pk_preview", { account_id: accountId, job_id: jobId }),
  submitWordPk: (accountId: string, jobId: string, answers: WordPkAnswer[]) =>
    invokeCommand("word_pk_submit", {
      account_id: accountId,
      job_id: jobId,
      answers,
    }),

  getReadAloud: (accountId: string, jobId: string) =>
    invokeCommand("read_aloud_get", { account_id: accountId, job_id: jobId }),
  uploadRecording: (
    accountId: string,
    jobId: string,
    questionId: string,
    fileName: string,
    dataBase64: string,
  ) =>
    invokeCommand("read_aloud_upload", {
      account_id: accountId,
      job_id: jobId,
      question_id: questionId,
      file_name: fileName,
      data_base64: dataBase64,
    }),
  submitReadAloud: (accountId: string, jobId: string, dryRun: boolean) =>
    invokeCommand("read_aloud_submit", {
      account_id: accountId,
      job_id: jobId,
      dry_run: dryRun,
    }),

  listWrongNotes: (
    accountId: string,
    page: number,
    pageSize: number,
    courseId?: string,
  ) =>
    invokeCommand("wrong_notes_list", {
      account_id: accountId,
      page,
      page_size: pageSize,
      course_id: courseId,
    }),
  previewWrongNoteCleanup: (accountId: string, questionIds: string[]) =>
    invokeCommand("wrong_notes_preview", {
      account_id: accountId,
      question_ids: questionIds,
    }),
  cleanupWrongNote: (accountId: string, questionId: string) =>
    invokeCommand("wrong_notes_cleanup", {
      account_id: accountId,
      question_id: questionId,
    }),

  getSettings: () => invokeCommand("settings_get", {}),
  updateSettings: (settings: Settings) =>
    invokeCommand("settings_update", { settings }),
  clearCache: () => invokeCommand("cache_clear", {}),
};
