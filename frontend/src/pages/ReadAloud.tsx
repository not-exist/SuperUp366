import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import { isTauriRuntime } from "../api/invoke";
import type {
  ReadAloudPreview,
  ReadAloudSlot,
  ReadAloudSubmit,
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
  recordUrl?: string;
  error?: string;
}

const UPLOAD_LABELS: Record<UploadStatus, string> = {
  idle: "未选择",
  validating: "校验中",
  ready: "待上传",
  uploading: "上传中",
  uploaded: "已上传",
  failed: "失败",
};

const UPLOAD_BADGE: Record<UploadStatus, string> = {
  idle: "badge--unknown",
  validating: "badge--pending",
  ready: "badge--pending",
  uploading: "badge--pending",
  uploaded: "badge--finished",
  failed: "badge--expired",
};

/** Pick one local file. Tauri returns a real path; the browser preview cannot. */
async function pickMp3(): Promise<{ path: string; name: string } | null> {
  if (isTauriRuntime) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "MP3", extensions: ["mp3"] }],
    });
    if (typeof selected !== "string") return null;
    const name = selected.split(/[\\/]/).pop() ?? selected;
    return { path: selected, name };
  }

  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".mp3,audio/mpeg";
    // The input is attached so the browser (and Playwright's file chooser)
    // treats the click as a real user gesture; it is removed on teardown.
    const finish = (value: { path: string; name: string } | null) => {
      input.remove();
      resolve(value);
    };
    input.onchange = () => {
      const file = input.files?.[0];
      finish(file ? { path: file.name, name: file.name } : null);
    };
    input.addEventListener("cancel", () => finish(null), { once: true });
    input.style.display = "none";
    document.body.append(input);
    input.click();
  });
}

export function ReadAloud() {
  const { jobId } = useParams();
  const account = useAccount();
  const queryClient = useQueryClient();
  const [uploads, setUploads] = useState<Record<string, LocalUpload>>({});
  const [scores, setScores] = useState<Record<string, number>>({});
  const [preview, setPreview] = useState<ReadAloudPreview | null>(null);
  const [confirmed, setConfirmed] = useState<ReadAloudSubmit | null>(null);

  const task = useQuery({
    queryKey: ["read-aloud", account.account_id, jobId],
    queryFn: () => api.getReadAloud(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  const slots = task.data?.slots ?? [];
  const submitted = task.data?.submitted ?? false;

  function statusFor(slot: ReadAloudSlot): UploadStatus {
    return uploads[slot.question_id]?.status ?? (slot.record_url ? "uploaded" : "idle");
  }

  function recordUrlFor(slot: ReadAloudSlot): string | null {
    return uploads[slot.question_id]?.recordUrl ?? slot.record_url;
  }

  function scoreFor(slot: ReadAloudSlot): number {
    return scores[slot.question_id] ?? slot.score;
  }

  const uploadedCount = slots.filter((slot) => statusFor(slot) === "uploaded").length;
  const allUploaded = slots.length > 0 && uploadedCount === slots.length;

  async function handlePick(slot: ReadAloudSlot) {
    const picked = await pickMp3();
    if (!picked) return;
    if (!picked.name.toLowerCase().endsWith(".mp3")) {
      setUploads((previous) => ({
        ...previous,
        [slot.question_id]: { status: "failed", fileName: picked.name, error: "只接受 mp3 文件" },
      }));
      return;
    }

    setUploads((previous) => ({
      ...previous,
      [slot.question_id]: { status: "uploading", fileName: picked.name },
    }));
    try {
      const result = await api.uploadRecording(
        account.account_id,
        jobId ?? "",
        slot.question_id,
        picked.path,
        picked.name,
      );
      setUploads((previous) => ({
        ...previous,
        [slot.question_id]: {
          status: "uploaded",
          fileName: result.file_name,
          recordUrl: result.record_url,
        },
      }));
    } catch (error) {
      setUploads((previous) => ({
        ...previous,
        [slot.question_id]: {
          status: "failed",
          fileName: picked.name,
          error: errorMessage(error),
        },
      }));
    }
  }

  const previewMutation = useMutation({
    mutationFn: () =>
      api.previewReadAloud(
        account.account_id,
        jobId ?? "",
        slots.map((slot) => ({ question_id: slot.question_id, score: scoreFor(slot) })),
      ),
    onSuccess: (data) => setPreview(data),
  });

  const submitMutation = useMutation({
    mutationFn: () => api.submitReadAloud(preview?.confirm_token ?? ""),
    onSuccess: (data) => {
      setConfirmed(data);
      setPreview(null);
      void queryClient.invalidateQueries({
        queryKey: ["read-aloud", account.account_id, jobId],
      });
    },
  });

  return (
    <div className="page" data-testid="readaloud-page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{task.data?.title ?? "朗读跟读"}</h1>
          <p className="page-subtitle">
            {task.data
              ? `自动识别：共 ${task.data.question_count} 题 / ${task.data.sentence_count} 句 / 满分 ${task.data.total_score}`
              : "正在读取题目…"}
          </p>
        </div>
        <div className="page-actions">
          <Link to="/jobs" className="button">
            <ArrowLeftIcon />
            返回作业
          </Link>
        </div>
      </header>

      {!jobId && <EmptyState title="缺少作业编号" description="请从作业列表重新进入。" />}
      {task.isPending && Boolean(jobId) && <LoadingState label="正在解析题目…" />}
      {task.isError && <ErrorState error={task.error} onRetry={() => task.refetch()} />}

      {task.data?.expired && !submitted && (
        <div className="banner banner--warning" role="note">
          <strong>已过期</strong>
          <span>服务端实测仍接受补交，可以继续上传并提交。</span>
        </div>
      )}

      {submitted && (
        <div className="banner banner--success" role="status">
          <strong>已提交</strong>
          <span>该作业的朗读成绩已经提交，服务端可能继续按录音复评。</span>
        </div>
      )}

      {confirmed && (
        <div
          className="banner banner--success"
          role="status"
          data-testid="readaloud-result"
        >
          <strong>{confirmed.verified ? "提交成功" : "已发送，等待服务端刷新"}</strong>
          <span>{confirmed.message}</span>
        </div>
      )}

      {task.isSuccess && slots.length === 0 && (
        <EmptyState
          title="没有跟读题目"
          description="该作业的题目包里没有 question_type = 9 的题目。"
        />
      )}

      {task.isSuccess && slots.length > 0 && (
        <>
          <div className="overall-row">
            <div style={{ flex: "1 1 240px" }}>
              <div className="field-hint">
                已上传 {uploadedCount} / {slots.length}
              </div>
              <div className="progress-track" aria-hidden="true">
                <div
                  className="progress-fill"
                  style={{ width: `${percent(uploadedCount, slots.length)}%` }}
                />
              </div>
            </div>
            <button
              type="button"
              className="button button--primary"
              data-testid="readaloud-preview"
              onClick={() => previewMutation.mutate()}
              disabled={
                !allUploaded || submitted || previewMutation.isPending || confirmed !== null
              }
            >
              <UploadIcon />
              {previewMutation.isPending ? "准备中…" : "预览并提交"}
            </button>
          </div>

          {previewMutation.isError && (
            <div className="banner banner--danger" role="alert">
              {errorMessage(previewMutation.error)}
            </div>
          )}
          {submitMutation.isError && (
            <div className="banner banner--danger" role="alert">
              {errorMessage(submitMutation.error)}
            </div>
          )}

          <div className="banner banner--info" role="note">
            <div>
              <strong>计分说明</strong>
              <span>
                本机没有厂商的语音评分引擎，上报分数由你填写；服务端收到后可能按录音重新评分。
              </span>
            </div>
          </div>

          <div className="stack">
            {slots.map((slot) => {
              const status = statusFor(slot);
              const local = uploads[slot.question_id];
              const recordUrl = recordUrlFor(slot);
              return (
                <article
                  key={slot.question_id}
                  className="read-card"
                  data-testid={`readaloud-slot-${slot.number}`}
                >
                  <div className="read-head">
                    <span className="question-no">第 {slot.number} 题</span>
                    <span
                      className={`badge ${UPLOAD_BADGE[status]}`}
                      data-testid={`readaloud-status-${slot.number}`}
                    >
                      <span className="badge__dot" aria-hidden="true" />
                      {UPLOAD_LABELS[status]}
                    </span>
                  </div>
                  <p className="read-sentence">{slot.sentence}</p>
                  <p className="field-hint">
                    {slot.sentences.length} 句 · 参考时长 {slot.duration}s · 满分 {slot.score}
                  </p>
                  {slot.audio_url && <audio controls src={slot.audio_url} preload="none" />}
                  <div className="read-controls">
                    <button
                      type="button"
                      className="button"
                      data-testid={`readaloud-pick-${slot.number}`}
                      onClick={() => void handlePick(slot)}
                      disabled={submitted || status === "uploading"}
                    >
                      选择 mp3
                    </button>
                    <label className="field-label" htmlFor={`score-${slot.question_id}`}>
                      上报分数
                    </label>
                    <input
                      id={`score-${slot.question_id}`}
                      className="input"
                      type="number"
                      min={0}
                      max={slot.score}
                      step={0.5}
                      value={scoreFor(slot)}
                      disabled={submitted}
                      onChange={(event) => {
                        const value = Number(event.currentTarget.value);
                        setScores((previous) => ({
                          ...previous,
                          [slot.question_id]: Number.isFinite(value) ? value : 0,
                        }));
                      }}
                    />
                    {local?.fileName && <span className="field-hint">{local.fileName}</span>}
                  </div>
                  {local?.error && (
                    <p className="field-hint" role="alert">
                      {local.error}
                    </p>
                  )}
                  {recordUrl && <p className="field-hint">录音地址：{recordUrl}</p>}
                </article>
              );
            })}
          </div>
        </>
      )}

      <ConfirmDialog
        open={preview !== null}
        title="确认提交朗读成绩"
        summary={
          preview ? (
            <>
              <p>
                共 {preview.slots.length} 题，上报 {preview.reported_score} /{" "}
                {preview.total_score} 分。提交后同一作业不能重复提交。
              </p>
              <ul className="summary-list">
                {preview.slots.map((slot) => (
                  <li key={slot.question_id}>
                    第 {slot.number} 题 · {slot.score} 分 · {slot.record_url}
                  </li>
                ))}
              </ul>
            </>
          ) : null
        }
        confirmLabel="确认提交"
        tone="danger"
        busy={submitMutation.isPending}
        onConfirm={() => submitMutation.mutate()}
        onCancel={() => {
          if (!submitMutation.isPending) setPreview(null);
        }}
      />
    </div>
  );
}
