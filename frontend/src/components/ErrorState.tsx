import { isApiError } from "../api/invoke";
import { AlertIcon, RefreshIcon } from "./Icons";

function normalize(error: unknown): {
  code: string;
  message: string;
  retryable: boolean;
} {
  if (isApiError(error)) {
    return {
      code: error.code,
      message: error.message,
      retryable: error.retryable,
    };
  }
  if (error instanceof Error) {
    return { code: "unknown", message: error.message, retryable: false };
  }
  return { code: "unknown", message: "请求失败，请稍后重试", retryable: false };
}

export function ErrorState({
  error,
  onRetry,
  title = "加载失败",
}: {
  error: unknown;
  onRetry?: () => void;
  title?: string;
}) {
  const info = normalize(error);
  const canRetry = info.retryable || info.code === "unknown";

  return (
    <div className="state-block error-state" role="alert">
      <span className="state-icon">
        <AlertIcon width={22} height={22} />
      </span>
      <p className="state-title">{title}</p>
      <p className="state-text">{info.message}</p>
      <p className="field-hint">错误码：{info.code}</p>
      {onRetry && canRetry && (
        <button type="button" className="button" onClick={onRetry}>
          <RefreshIcon />
          重试
        </button>
      )}
    </div>
  );
}
