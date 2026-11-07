import { Link, useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { AnswerStatus } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";

const ANSWER_LABELS: Record<AnswerStatus, string> = {
  right: "正确",
  wrong: "错误",
  partial: "部分正确",
  unanswered: "未作答",
};

const ANSWER_BADGE: Record<AnswerStatus, string> = {
  right: "badge--finished",
  wrong: "badge--expired",
  partial: "badge--returned",
  unanswered: "badge--unknown",
};

const ANSWER_CLASS: Record<AnswerStatus, string> = {
  right: "answer-value--right",
  wrong: "answer-value--wrong",
  partial: "answer-value--partial",
  unanswered: "answer-value--muted",
};

export function AnswerView() {
  const { jobId } = useParams();
  const account = useAccount();

  const answer = useQuery({
    queryKey: ["answer", account.account_id, jobId],
    queryFn: () => api.getAnswer(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  const questions = answer.data?.questions ?? [];
  const counts = questions.reduce(
    (acc, question) => {
      acc[question.status] += 1;
      return acc;
    },
    { right: 0, wrong: 0, partial: 0, unanswered: 0 },
  );

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{answer.data?.title ?? "作业答案"}</h1>
          <p className="page-subtitle">
            {answer.data
              ? `得分 ${answer.data.score ?? "—"} / ${answer.data.max_score} · 共 ${questions.length} 题`
              : "逐题查看作答与标准答案。"}
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

      {answer.isPending && Boolean(jobId) && (
        <LoadingState label="正在加载答案…" />
      )}

      {answer.isError && (
        <ErrorState error={answer.error} onRetry={() => answer.refetch()} />
      )}

      {answer.isSuccess && questions.length === 0 && (
        <EmptyState
          title="暂无答案数据"
          description="该作业可能尚未批改，或答案接口还没有返回题目。"
        />
      )}

      {answer.isSuccess && questions.length > 0 && (
        <>
          <section className="answer-summary" aria-label="答题统计">
            <div className="stat">
              <div className="stat-label">正确</div>
              <div className="stat-value">{counts.right}</div>
            </div>
            <div className="stat">
              <div className="stat-label">错误</div>
              <div className="stat-value">{counts.wrong}</div>
            </div>
            <div className="stat">
              <div className="stat-label">部分正确</div>
              <div className="stat-value">{counts.partial}</div>
            </div>
            <div className="stat">
              <div className="stat-label">未作答</div>
              <div className="stat-value">{counts.unanswered}</div>
            </div>
          </section>

          <div className="stack">
            {questions.map((question) => (
              <article key={question.question_id} className="question-card">
                <div className="question-head">
                  <span className="question-no">第 {question.number} 题</span>
                  <span className={`badge ${ANSWER_BADGE[question.status]}`}>
                    {ANSWER_LABELS[question.status]}
                  </span>
                </div>
                <p className="question-stem">{question.stem}</p>
                <div className="answer-grid">
                  <div>
                    <div className="stat-label">你的答案</div>
                    <div
                      className={`answer-value ${ANSWER_CLASS[question.status]}`}
                    >
                      {question.student_answer || "（空）"}
                    </div>
                  </div>
                  <div>
                    <div className="stat-label">正确答案</div>
                    <div className="answer-value">
                      {question.correct_answer || "—"}
                    </div>
                  </div>
                  <div>
                    <div className="stat-label">得分</div>
                    <div className="answer-value">
                      {question.score ?? "—"} / {question.max_score}
                    </div>
                  </div>
                </div>
              </article>
            ))}
          </div>

          <details className="raw-details">
            <summary>原始试卷 XML</summary>
            <pre>{answer.data.paper_xml}</pre>
          </details>

          <p className="field-hint" style={{ marginTop: 12 }}>
            任务 ID：{answer.data.task_id}（答案接口使用 contentId，不是作业 ID）
          </p>
        </>
      )}
    </div>
  );
}
