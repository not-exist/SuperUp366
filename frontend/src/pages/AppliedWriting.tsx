import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { AppliedWritingPreview, AppliedWritingSubmit } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, UploadIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { countWords, errorMessage, writingCharacterCount } from "../utils";

/** Seconds spent on the paper, at least one and at most one day. */
function studySeconds(startedAt: number | null): number {
  if (startedAt === null) return 1;
  return Math.min(86_400, Math.max(1, Math.floor((Date.now() - startedAt) / 1000)));
}

export function AppliedWriting() {
  const { jobId } = useParams();
  const account = useAccount();
  const queryClient = useQueryClient();
  const [texts, setTexts] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<AppliedWritingPreview | null>(null);
  const [confirmed, setConfirmed] = useState<AppliedWritingSubmit | null>(null);
  const startedAt = useRef<number | null>(null);

  const task = useQuery({
    queryKey: ["applied-writing", account.account_id, jobId],
    queryFn: () => api.getAppliedWriting(account.account_id, jobId ?? ""),
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
  const answers = questions.map((question) => ({
    question_id: question.question_id,
    text: texts[question.question_id] ?? "",
  }));
  const allFilled = questions.length > 0 && answers.every((answer) => answer.text.trim() !== "");
  const totalWords = answers.reduce((sum, answer) => sum + countWords(answer.text), 0);

  const previewMutation = useMutation({
    mutationFn: () =>
      api.previewAppliedWriting(
        account.account_id,
        jobId ?? "",
        answers,
        studySeconds(startedAt.current),
      ),
    onSuccess: (data) => {
      setConfirmed(null);
      setPreview(data);
    },
  });

  const submitMutation = useMutation({
    mutationFn: () => api.submitAppliedWriting(preview?.confirm_token ?? ""),
    onSuccess: (data) => {
      setConfirmed(data);
      setPreview(null);
      void queryClient.invalidateQueries({
        queryKey: ["applied-writing", account.account_id, jobId],
      });
      void queryClient.invalidateQueries({
        queryKey: ["jobs", account.account_id],
      });
    },
  });

  return (
    <div className="page" data-testid="appliedwriting-page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{task.data?.title ?? "应用文写作"}</h1>
          <p className="page-subtitle">
            {task.data
              ? `检测到 ${task.data.questions.length} 道应用文写作题 · 满分 ${task.data.total_score}`
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
        <LoadingState label="正在解析应用文题目…" />
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

      {submitted && (
        <div className="banner banner--warning" role="note">
          <strong>已提交 · 可重新提交</strong>
          <span>
            该作业已有提交记录。重新提交可能被服务端拒绝（已提交/次数用尽），也可能按任务更新成绩；以返回结果为准。
          </span>
        </div>
      )}

      {confirmed && (
        <div
          className="banner banner--success"
          role="status"
          data-testid="writing-result"
        >
          <strong>
            {confirmed.resubmitted
              ? "重新提交成功"
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
          title="没有应用文写作题"
          description="该作业的试卷里没有 question_type 14 的题目。"
        />
      )}

      {task.isSuccess && questions.length > 0 && (
        <>
          <div className="overall-row">
            <div style={{ flex: "1 1 240px" }}>
              <div className="field-hint">
                已填写 {answers.filter((answer) => answer.text.trim() !== "").length} /{" "}
                {questions.length} 题 · 共 {totalWords} 词
              </div>
            </div>
            <button
              type="button"
              className="button button--primary"
              data-testid="writing-preview"
              onClick={() => previewMutation.mutate()}
              disabled={!allFilled || previewMutation.isPending}
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
                本机不生成作文分数；答案提交后由服务端批改。提交后仍可重新提交，重复提交的服务端行为以实际返回为准。
              </span>
            </div>
          </div>

          <div className="stack">
            {questions.map((question, index) => {
              const words = countWords(texts[question.question_id] ?? "");
              const characterCount = writingCharacterCount(texts[question.question_id] ?? "");
              return (
                <article
                  key={question.question_id}
                  className="read-card"
                  data-testid={`writing-question-${index + 1}`}
                >
                  <div className="read-head">
                    <span className="question-no">第 {question.number} 题</span>
                    <span className="field-hint">
                      满分 {question.score}
                      {question.word_num ? ` · 建议 ${question.word_num} 词` : ""}
                      {question.duration > 0 ? ` · 参考时长 ${question.duration}s` : ""}
                    </span>
                  </div>
                  <p className="writing-prompt">{question.prompt}</p>
                  <textarea
                    className="input writing-input"
                    data-testid={`writing-input-${index + 1}`}
                    maxLength={3000}
                    value={texts[question.question_id] ?? ""}
                    placeholder="在此输入英文作文"
                    onChange={(event) => {
                      const value = event.currentTarget.value;
                      setTexts((previous) => ({
                        ...previous,
                        [question.question_id]: value,
                      }));
                    }}
                  />
                  <div
                    className="field-hint"
                    data-testid={`writing-wordcount-${index + 1}`}
                  >
                    已输入单词：{words} · 字符：{characterCount} / 3000
                  </div>
                </article>
              );
            })}
          </div>
        </>
      )}

      <ConfirmDialog
        open={preview !== null}
        title={preview?.resubmit ? "确认重新提交应用文" : "确认提交应用文"}
        summary={
          preview ? (
            <>
              <p>
                {preview.title} · 共 {preview.answers.length} 题 · 满分{" "}
                {preview.total_score}。
                {preview.resubmit
                  ? "这是重新提交：服务端可能拒绝（已提交/次数用尽），也可能按任务更新成绩。"
                  : "作文由服务端批改；提交后仍可重新提交。"}
              </p>
              {preview.resubmit && (
                <p className="field-hint" role="note">
                  本次上报学习时长 {preview.seconds} 秒；服务端如何处理以实际返回为准。
                </p>
              )}
              <ul className="summary-list">
                {preview.answers.map((answer) => (
                  <li key={answer.question_id}>
                    第 {answer.number} 题 · {countWords(answer.text)} 词 ·{" "}
                    {answer.text.slice(0, 80)}
                    {answer.text.length > 80 ? "…" : ""}
                  </li>
                ))}
              </ul>
              <details className="raw-details">
                <summary>查看提交的 answerData XML</summary>
                <pre data-testid="writing-preview-xml">{preview.answer_data}</pre>
              </details>
            </>
          ) : null
        }
        confirmLabel={preview?.resubmit ? "确认重新提交" : "确认提交"}
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
