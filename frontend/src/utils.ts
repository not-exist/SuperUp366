import type { JobKind } from "./api/types";
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
