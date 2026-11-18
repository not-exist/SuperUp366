import type { BookJob, JobKind } from "./api/types";
import { isApiError } from "./api/invoke";

export function errorMessage(error: unknown): string {
  if (isApiError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return "请求失败，请稍后重试";
}

export function formatDateTime(value: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

export function isOverdue(value: string | null): boolean {
  if (!value) return false;
  const date = new Date(value);
  return !Number.isNaN(date.getTime()) && date.getTime() < Date.now();
}

export function percent(current: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((current / total) * 100)));
}

/**
 * Seconds spent on a paper: at least one, at most one day.
 *
 * Shared by the writing and dictation pages, which send `duration_sec`.
 */
export function studySeconds(startedAt: number | null): number {
  if (startedAt === null) return 1;
  return Math.min(86_400, Math.max(1, Math.floor((Date.now() - startedAt) / 1000)));
}

export function writingCharacterCount(text: string): number {
  return text.replace(/\n+/g, "").length;
}

/** Count English words the way the writing editor does: punctuation splits. */
export function countWords(text: string): number {
  const cleaned = text.replace(/[\r\n\t,，;；!！.。?？:：'"“”‘’/\\()[\]{}<>_+=*\-%$¥@#&£€]/g, " ");
  return cleaned
    .trim()
    .split(/\s+/)
    .filter((word) => /[\w]/.test(word)).length;
}

export function initialOf(name: string): string {
  const trimmed = name.trim();
  return trimmed ? trimmed.slice(0, 1).toUpperCase() : "U";
}

/** Route for a job row, chosen by kind and by the read-aloud marker. */
export function jobPath(job: {
  job_id: string;
  kind: JobKind;
  read_aloud: boolean;
}): string {
  if (job.kind === "word_pk") return `/jobs/${job.job_id}/wordpk`;
  if (job.read_aloud) return `/jobs/${job.job_id}/readaloud`;
  return `/jobs/${job.job_id}/answer`;
}

/** Route for a book assignment; the book view resolves its own ids. */
export function bookTaskPath(
  bookId: string,
  bookType: number,
  courseId: string,
  job: BookJob,
): string {
  const params = new URLSearchParams({
    type: String(bookType),
    course: courseId,
    kind: job.kind,
    name: job.name,
  });
  return `/books/${bookId}/tasks/${job.job_id}?${params.toString()}`;
}

/** Read a selected mp3 as base64 for the upload command. */
export async function fileToBase64(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}
