import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type {
  ReadAloudQuestion,
  SubmitResult,
  UploadStatus,
} from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, UploadIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage, fileToBase64, percent } from "../utils";

interface LocalUpload {
  status: UploadStatus;
  fileName?: string;
  error?: string;
}

const UPLOAD_LABELS: Record<UploadStatus, string> = {
  idle: "未选择",
  uploading: "上传中",
  uploaded: "已上传",
  failed: "上传失败",
};

const UPLOAD_BADGE: Record<UploadStatus, string> = {
  idle: "badge--unknown",
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
  const [result, setResult] = useState<SubmitResult | null>(null);

  const task = useQuery({
    queryKey: ["read-aloud", account.account_id, jobId],
    queryFn: () => api.getReadAloud(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });

  const dryRun = settings.data?.read_aloud_dry_run ?? true;
  const questions = task.data?.questions ?? [];

  function statusFor(question: ReadAloudQuestion): UploadStatus {
    return (
      uploads[question.question_id]?.status ??
      (question.record_url ? "uploaded" : "idle")
    );
  }

  const uploadedCount = questions.filter(
    (question) => statusFor(question) === "uploaded",
  ).length;
  const allUploaded =
    questions.length > 0 && uploadedCount === questions.length;

  const submit = useMutation({
    mutationFn: () =>
      api.submitReadAloud(account.account_id, jobId ?? "", dryRun),
    onSuccess: (data) => {
      setResult(data);
      setConfirmOpen(false);
      void queryClient.invalidateQueries({
        queryKey: ["read-aloud", account.account_id, jobId],
      });
    },
  });

  async function handleFile(question: ReadAloudQuestion, file: File) {
    const questionId = question.question_id;
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
      [questionId]: { status: "uploading", fileName: file.name },
    }));

    try {
      const dataBase64 = await fileToBase64(file);
      await api.uploadRecording(
        account.account_id,
        jobId ?? "",
        questionId,
        file.name,
        dataBase64,
      );
      setUploads((previous) => ({
        ...previous,
        [questionId]: { status: "uploaded", fileName: file.name },
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
              ? `共 ${questions.length} 题，每个问题上传一段 mp3`
              : "逐题选择并上传录音。"}
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
              : "演练模式已关闭，提交会调用正式成绩接口。"}
          </span>
        </div>
      </div>

      {result && (
        <div className="banner banner--success" role="status">
          <strong>{result.ok ? "操作成功" : "操作失败"}</strong>
          <span>{result.message}</span>
        </div>
      )}

      {submit.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(submit.error)}
        </div>
      )}

      {task.isSuccess && questions.length === 0 && (
        <EmptyState
          title="暂无朗读题目"
          description="该作业不包含 question_type = 9 的题目。"
        />
      )}

      {task.isSuccess && questions.length > 0 && (
        <>
          <div className="overall-row">
            <div style={{ flex: "1 1 240px" }}>
              <div className="field-hint">
                已上传 {uploadedCount} / {questions.length}
              </div>
              <div className="progress-track" aria-hidden="true">
                <div
                  className="progress-fill"
                  style={{
                    width: `${percent(uploadedCount, questions.length)}%`,
                  }}
                />
              </div>
            </div>
            <button
              type="button"
              className="button button--primary"
              onClick={() => setConfirmOpen(true)}
              disabled={!allUploaded || submit.isPending || result?.ok === true}
            >
              <UploadIcon />
              {result?.ok ? "已提交" : dryRun ? "演练提交" : "提交成绩"}
            </button>
          </div>

          <div className="stack">
            {questions.map((question) => {
              const status = statusFor(question);
              const local = uploads[question.question_id];
              const inputId = `file-${question.question_id}`;
              return (
                <article key={question.question_id} className="read-card">
                  <div className="read-head">
                    <span className="question-no">第 {question.number} 题</span>
                    <span className={`badge ${UPLOAD_BADGE[status]}`}>
                      <span className="badge__dot" aria-hidden="true" />
                      {UPLOAD_LABELS[status]}
                    </span>
                  </div>
                  <p className="read-sentence">{question.sentence}</p>
                  {question.audio_url && (
                    <audio controls src={question.audio_url} preload="none" />
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
                        if (file) void handleFile(question, file);
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
        title={dryRun ? "演练提交" : "提交朗读成绩"}
        summary={
          <>
            <p>
              将处理 {uploadedCount} 题录音。
              {dryRun
                ? "本次只做上传校验与演练，不会写入成绩。"
                : "本次将调用正式成绩接口，提交后可能无法修改。"}
            </p>
            <ul className="summary-list">
              {questions
                .filter((question) => statusFor(question) === "uploaded")
                .slice(0, 8)
                .map((question) => (
                  <li key={question.question_id}>
                    第 {question.number} 题 · {question.sentence}
                  </li>
                ))}
            </ul>
          </>
        }
        confirmLabel={dryRun ? "开始演练" : "确认提交"}
        tone={dryRun ? "default" : "danger"}
        busy={submit.isPending}
        onConfirm={() => submit.mutate()}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}
