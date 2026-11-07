import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type {
  ReadAloudPreview,
  ReadAloudSlot,
  UploadStatus,
} from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, UploadIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage, percent } from "../utils";

interface LocalUpload {
  status: UploadStatus;
  fileName?: string;
  error?: string;
}

// The OSS transfer is not wired yet, so a validated file is "ready" and must
// never be presented as uploaded.
const UPLOAD_LABELS: Record<UploadStatus, string> = {
  idle: "未选择",
  validating: "校验中",
  ready: "已校验（未上传）",
  uploading: "上传中",
  uploaded: "已上传",
  failed: "校验失败",
};

const UPLOAD_BADGE: Record<UploadStatus, string> = {
  idle: "badge--unknown",
  validating: "badge--pending",
  ready: "badge--pending",
  uploading: "badge--pending",
  uploaded: "badge--finished",
  failed: "badge--expired",
};

export function ReadAloud() {
  const { jobId } = useParams();
  const account = useAccount();
  const queryClient = useQueryClient();
  const [uploads, setUploads] = useState<Record<string, LocalUpload>>({});
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [result, setResult] = useState<ReadAloudPreview | null>(null);

  const task = useQuery({
    queryKey: ["read-aloud", account.account_id, jobId],
    queryFn: () => api.getReadAloud(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  // The score path is closed until the payload shape is confirmed, so this
  // is always a dry run regardless of the settings toggle.
  const dryRun = true;
  const slots = task.data?.slots ?? [];

  function statusFor(slot: ReadAloudSlot): UploadStatus {
    return (
      uploads[slot.question_id]?.status ??
      (slot.record_url ? "uploaded" : "idle")
    );
  }

  const readyCount = slots.filter((slot) => statusFor(slot) === "ready").length;
  const allReady = slots.length > 0 && readyCount === slots.length;

  const submit = useMutation({
    mutationFn: () =>
      api.submitReadAloud(account.account_id, jobId ?? "", true),
    onSuccess: (data) => {
      setResult(data);
      setConfirmOpen(false);
      void queryClient.invalidateQueries({
        queryKey: ["read-aloud", account.account_id, jobId],
      });
    },
  });

  async function handleFile(slot: ReadAloudSlot, file: File) {
    const questionId = slot.question_id;
    if (!file.name.toLowerCase().endsWith(".mp3")) {
      setUploads((previous) => ({
        ...previous,
        [questionId]: {
          status: "failed",
          fileName: file.name,
          error: "只接受 mp3 文件",
        },
      }));
      return;
    }

    setUploads((previous) => ({
      ...previous,
      [questionId]: { status: "validating", fileName: file.name },
    }));

    try {
      await api.uploadRecording(
        account.account_id,
        jobId ?? "",
        questionId,
        file.name,
        file.name,
      );
      setUploads((previous) => ({
        ...previous,
        // "ready", not "uploaded": the bytes never left the machine.
        [questionId]: { status: "ready", fileName: file.name },
      }));
    } catch (error) {
      setUploads((previous) => ({
        ...previous,
        [questionId]: {
          status: "failed",
          fileName: file.name,
          error: errorMessage(error),
        },
      }));
    }
  }

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{task.data?.title ?? "朗读跟读"}</h1>
          <p className="page-subtitle">
            {task.data
              ? `共 ${slots.length} 段录音，每段对应一道跟读题`
              : "逐题选择录音文件。"}
          </p>
        </div>
        <div className="page-actions">
          <Link to="/jobs" className="button">
            <ArrowLeftIcon />
            返回作业
          </Link>
        </div>
      </header>

      {!jobId && (
        <EmptyState title="缺少作业编号" description="请从作业列表重新进入。" />
      )}

      {task.isPending && Boolean(jobId) && (
        <LoadingState label="正在加载朗读任务…" />
      )}

      {task.isError && (
        <ErrorState error={task.error} onRetry={() => task.refetch()} />
      )}

      <div className="banner banner--info" role="note">
        <div>
          <strong>提交说明</strong>
          <span>
            {dryRun
              ? "当前为演练模式：只上传并校验录音，不向服务器提交成绩。可在设置中关闭演练。"
              : ""}
          </span>
        </div>
      </div>

      {result && (
        <div className="banner banner--success" role="status">
          <strong>已生成提交预览</strong>
          <span>
            预览仅包含将要发送的内容，尚未向服务器提交成绩
            {result.dry_run ? "" : "（注意：本次并非演练）"}。
          </span>
        </div>
      )}

      {submit.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(submit.error)}
        </div>
      )}

      {task.isSuccess && slots.length === 0 && (
        <EmptyState
          title="暂无朗读题目"
          description="该作业不包含 question_type = 9 的题目。"
        />
      )}

      {task.isSuccess && slots.length > 0 && (
        <>
          <div className="overall-row">
            <div style={{ flex: "1 1 240px" }}>
              <div className="field-hint">
                已校验 {readyCount} / {slots.length}
              </div>
              <div className="progress-track" aria-hidden="true">
                <div
                  className="progress-fill"
                  style={{
                    width: `${percent(readyCount, slots.length)}%`,
                  }}
                />
              </div>
            </div>
            <button
              type="button"
              className="button button--primary"
              onClick={() => setConfirmOpen(true)}
              disabled={!allReady || submit.isPending || result !== null}
            >
              <UploadIcon />
              {result ? "已生成预览" : "生成提交预览"}
            </button>
          </div>

          <div className="stack">
            {slots.map((slot) => {
              const status = statusFor(slot);
              const local = uploads[slot.question_id];
              const inputId = `file-${slot.question_id}`;
              return (
                <article key={slot.question_id} className="read-card">
                  <div className="read-head">
                    <span className="question-no">第 {slot.number} 题</span>
                    <span className={`badge ${UPLOAD_BADGE[status]}`}>
                      <span className="badge__dot" aria-hidden="true" />
                      {UPLOAD_LABELS[status]}
                    </span>
                  </div>
                  <p className="read-sentence">{slot.sentence}</p>
                  {slot.audio_url && (
                    <audio controls src={slot.audio_url} preload="none" />
                  )}
                  <div className="read-controls">
                    <label className="field-label" htmlFor={inputId}>
                      选择 mp3
                    </label>
                    <input
                      id={inputId}
                      className="file-input"
                      type="file"
                      accept="audio/mpeg,.mp3"
                      disabled={status === "uploading"}
                      onChange={(event) => {
                        const file = event.currentTarget.files?.[0];
                        event.currentTarget.value = "";
                        if (file) void handleFile(slot, file);
                      }}
                    />
                    {local?.fileName && (
                      <span className="field-hint">{local.fileName}</span>
                    )}
                  </div>
                  {local?.error && (
                    <p className="field-hint" role="alert">
                      {local.error}
                    </p>
                  )}
                  {status === "uploaded" && (
                    <p className="field-hint">
                      录音已上传，可重新选择文件覆盖（同一题只使用最后一段）。
                    </p>
                  )}
                </article>
              );
            })}
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirmOpen}
        title="生成提交预览"
        summary={
          <>
            <p>
              将处理 {readyCount} 段录音。此操作只会生成将要发送的内容，不会
              向服务器提交成绩。
            </p>
            <ul className="summary-list">
              {slots
                .filter((slot: ReadAloudSlot) => statusFor(slot) === "ready")
                .slice(0, 8)
                .map((slot) => (
                  <li key={slot.question_id}>
                    第 {slot.number} 题 · {slot.sentence}
                  </li>
                ))}
            </ul>
          </>
        }
        confirmLabel="生成预览"
        tone="default"
        busy={submit.isPending}
        onConfirm={() => submit.mutate()}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}
