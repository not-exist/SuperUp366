import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { DictationPreview, DictationSubmit } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, UploadIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage, studySeconds } from "../utils";

export function Dictation() {
  const { jobId } = useParams();
  const account = useAccount();
  const queryClient = useQueryClient();
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<DictationPreview | null>(null);
  const [confirmed, setConfirmed] = useState<DictationSubmit | null>(null);
  const startedAt = useRef<number | null>(null);

  const task = useQuery({
    queryKey: ["dictation", account.account_id, jobId],
    queryFn: () => api.getDictation(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  // The route element is reused when the job id changes; drop the previous
  // job's draft, preview and timer before loading the new paper.
  useEffect(() => {
    startedAt.current = null;
    setTexts({});
    setPreview(null);
    setConfirmed(null);
  }, [jobId]);

  useEffect(() => {
    if (task.data && startedAt.current === null) {
      startedAt.current = Date.now();
    }
  }, [task.data]);

  const questions = task.data?.questions ?? [];
  const submitted = task.data?.submitted ?? false;
  const retestSupported = task.data?.retest_supported ?? false;
  // A submitted paper with no redo support is read-only; one that supports
  // retest stays editable because the submission resets the old answers first.
  const locked = submitted && !retestSupported;
  const answers = questions.map((question) => ({
    question_id: question.question_id,
    text: texts[question.question_id] ?? "",
  }));
  const filledCount = answers.filter((answer) => answer.text.trim() !== "").length;
  const allFilled = questions.length > 0 && filledCount === questions.length;

  const previewMutation = useMutation({
    mutationFn: () =>
      api.previewDictation(
        account.account_id,
        jobId ?? "",
        answers,
        studySeconds(startedAt.current),
      ),
    onSuccess: (data) => setPreview(data),
  });

  const submitMutation = useMutation({
    mutationFn: () => api.submitDictation(preview?.confirm_token ?? ""),
    onSuccess: (data) => {
      setConfirmed(data);
      setPreview(null);
      void queryClient.invalidateQueries({
        queryKey: ["dictation", account.account_id, jobId],
      });
      void queryClient.invalidateQueries({
        queryKey: ["answer", account.account_id, jobId],
      });
      void queryClient.invalidateQueries({
        queryKey: ["jobs", account.account_id],
      });
    },
  });

  return (
    <div className="page" data-testid="dictation-page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{task.data?.title ?? "默写作业"}</h1>
          <p className="page-subtitle">
            {task.data
              ? `检测到 ${task.data.questions.length} 道默写题 · 满分 ${task.data.total_score}${task.data.other_questions > 0 ? ` · 另有 ${task.data.other_questions} 道其他题型` : ""}`
              : "正在读取试卷…"}
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
        <LoadingState label="正在解析默写题目…" />
      )}
      {task.isError && (
        <ErrorState error={task.error} onRetry={() => task.refetch()} />
      )}

      {task.data?.expired && !submitted && (
        <div className="banner banner--warning" role="note">
          <strong>已过期</strong>
          <span>服务端实测仍可能接受补交，请自行确认后再提交。</span>
        </div>
      )}

      {submitted && !retestSupported && (
        <div
          className="banner banner--success"
          role="status"
          data-testid="dictation-submitted"
        >
          <strong>已提交</strong>
          <span>
            该作业已经提交，不能再次提交。
            {task.data?.retest_reason ? ` ${task.data.retest_reason}` : ""}
          </span>
        </div>
      )}

      {submitted && retestSupported && (
        <div
          className="banner banner--info"
          role="note"
          data-testid="dictation-retest-notice"
        >
          <div>
            <strong>支持重做</strong>
            <span>
              该作业支持重做，提交时会先重置上一次作答。
              {task.data?.retest_reason ? ` ${task.data.retest_reason}` : ""}
            </span>
          </div>
        </div>
      )}

      {confirmed && (
        <div
          className="banner banner--success"
          role="status"
          data-testid="dictation-result"
        >
          <strong>
            {confirmed.retested
              ? "已重做并重新提交"
              : confirmed.verified
                ? "提交成功"
                : "已发送，等待服务端刷新"}
          </strong>
          <span>
            {confirmed.message}
            {confirmed.score != null ? ` · 当前分数 ${confirmed.score}` : ""}
          </span>
        </div>
      )}

      {task.isSuccess && questions.length === 0 && (
        <EmptyState
          title={
            task.data.other_questions > 0
              ? "这是混合试卷"
              : "没有默写题"
          }
          description={
            task.data.other_questions > 0
              ? `该作业还包含 ${task.data.other_questions} 道其他类型的题目，本客户端只提交纯默写卷。`
              : "该作业的试卷里没有 question_type 19/20 的题目。"
          }
        />
      )}

      {task.isSuccess && questions.length > 0 && (
        <>
          <div className="overall-row">
            <div style={{ flex: "1 1 240px" }}>
              <div className="field-hint" data-testid="dictation-progress">
                已填写 {filledCount} / {questions.length} 题
              </div>
            </div>
            <button
              type="button"
              className="button button--primary"
              data-testid="dictation-preview"
              onClick={() => previewMutation.mutate()}
              disabled={
                !allFilled || locked || previewMutation.isPending || confirmed !== null
              }
            >
              <UploadIcon />
              {previewMutation.isPending
                ? "准备中…"
                : submitted && retestSupported
                  ? "预览并重新提交"
                  : "预览并提交"}
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
              <strong>判分说明</strong>
              <span>
                {submitted && retestSupported
                  ? "本机不生成默写分数；答案提交后由服务端判分。本次提交会先重置上一次作答。"
                  : "本机不生成默写分数；答案提交后由服务端判分。"}
              </span>
            </div>
          </div>

          <div className="stack">
            {questions.map((question, index) => (
              <article
                key={question.question_id}
                className="read-card"
                data-testid={`dictation-question-${index + 1}`}
              >
                <div className="read-head">
                  <span className="question-no">第 {question.number} 题</span>
                  <span className="field-hint">
                    满分 {question.score}
                    {question.duration > 0 ? ` · 参考时长 ${question.duration}s` : ""}
                  </span>
                </div>
                <p className="writing-prompt">{question.prompt}</p>
                <textarea
                  className="input writing-input"
                  data-testid={`dictation-input-${index + 1}`}
                  value={texts[question.question_id] ?? ""}
                  placeholder="在此输入默写内容"
                  disabled={locked || confirmed !== null}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setTexts((previous) => ({
                      ...previous,
                      [question.question_id]: value,
                    }));
                  }}
                />
              </article>
            ))}
          </div>
        </>
      )}

      <ConfirmDialog
        open={preview !== null}
        title="确认提交默写"
        summary={
          preview ? (
            <>
              <p>
                {preview.title} · 共 {preview.answers.length} 题 · 满分{" "}
                {preview.total_score}。
                {preview.retest
                  ? "本次提交会先重置上一次作答，再写入新答案。"
                  : "提交后不能重复提交，默写由服务端判分。"}
              </p>
              {preview.retest && (
                <p data-testid="dictation-retest-confirm">
                  本次将先重置原作答，再提交新答案。
                </p>
              )}
              <ul className="summary-list">
                {preview.answers.map((answer) => (
                  <li key={answer.question_id}>
                    第 {answer.number} 题 · {answer.text.slice(0, 80)}
                    {answer.text.length > 80 ? "…" : ""}
                  </li>
                ))}
              </ul>
              <details className="raw-details">
                <summary>查看提交的 answerData XML</summary>
                <pre data-testid="dictation-preview-xml">{preview.answer_data}</pre>
              </details>
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
