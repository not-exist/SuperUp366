// Typed wrapper around Tauri's `invoke`.
//
// Every call goes through one place so error shapes stay uniform and so the
// same UI can run in a plain browser: when the Tauri runtime is absent the
// command is served by the mock backend instead.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { handleMock } from "./mock";
import type { CommandMap } from "./types";

/** Stable error model rendered by `ErrorState` and inline alerts. */
export class ApiError extends Error {
  readonly code: string;
  readonly retryable: boolean;

  constructor(code: string, message: string, retryable = false) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

export const isTauriRuntime =
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

function normalizeError(error: unknown): ApiError {
  if (error instanceof ApiError) {
    return error;
  }

  if (typeof error === "string") {
    // Tauri commands commonly reject with a JSON-encoded `DomainError`.
    try {
      return normalizeError(JSON.parse(error) as unknown);
    } catch {
      return new ApiError("unknown", error || "请求失败", false);
    }
  }

  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const code = typeof record.code === "string" ? record.code : "unknown";
    const message =
      typeof record.message === "string" ? record.message : "请求失败";
    const retryable =
      record.retryable === true ||
      code === "network_error" ||
      code === "timeout";
    return new ApiError(code, message, retryable);
  }

  return new ApiError("unknown", "未知错误", false);
}

export async function invokeCommand<K extends keyof CommandMap>(
  command: K,
  args: CommandMap[K]["args"],
): Promise<CommandMap[K]["result"]> {
  try {
    if (isTauriRuntime) {
      const result = await tauriInvoke(
        command,
        args as unknown as Record<string, unknown>,
      );
      return result as CommandMap[K]["result"];
    }
    return (await handleMock(command, args)) as CommandMap[K]["result"];
  } catch (error) {
    throw normalizeError(error);
  }
}
