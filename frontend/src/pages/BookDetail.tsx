import { Link, useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon } from "../components/Icons";
import { StatusBadge } from "../components/StatusBadge";
import { useAccount } from "../hooks/account";
import {
  formatDateTime,
  isOverdue,
  jobPath,
  percent,
} from "../utils";

export function BookDetail() {
  const { bookId } = useParams();
  const account = useAccount();

  const detail = useQuery({
    queryKey: ["book", account.account_id, bookId],
    queryFn: () => api.getBook(account.account_id, bookId ?? ""),
    enabled: Boolean(bookId),
  });

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">书籍详情</h1>
          <p className="page-subtitle">按章节查看同步作业与完成情况。</p>
        </div>
        <div className="page-actions">
          <Link to="/books" className="button">
            <ArrowLeftIcon />
            返回书籍
          </Link>
        </div>
      </header>

      {!bookId && (
        <EmptyState title="缺少书籍编号" description="请从书籍列表重新进入。" />
      )}

      {detail.isPending && Boolean(bookId) && (
        <LoadingState label="正在加载书籍目录…" />
      )}

      {detail.isError && (
        <ErrorState error={detail.error} onRetry={() => detail.refetch()} />
      )}

      {detail.isSuccess && (
        <>
          <section className="book-hero">
            <span className="book-cover" aria-hidden="true">
              {detail.data.name.slice(0, 1)}
            </span>
            <div>
              <h2 className="book-name">{detail.data.name}</h2>
              <p className="book-meta">
                {detail.data.subject}
                {detail.data.teacher_name
                  ? ` · ${detail.data.teacher_name}`
                  : ""}
              </p>
              <p className="book-meta">
                已完成 {detail.data.progress}/{detail.data.total}
              </p>
              <div className="progress-track" aria-hidden="true">
                <div
                  className="progress-fill"
                  style={{
                    width: `${percent(detail.data.progress, detail.data.total)}%`,
                  }}
                />
              </div>
            </div>
          </section>

          {detail.data.chapters.map((chapter) => (
            <section key={chapter.chapter_id} className="chapter">
              <h3 className="chapter-title">{chapter.title}</h3>
              {chapter.jobs.length === 0 ? (
                <p className="field-hint">本章暂无作业。</p>
              ) : (
                <>
                  <div className="job-list-head job-list-head--book" aria-hidden="true">
                    <span>作业</span>
                    <span>状态</span>
                    <span>分数</span>
                    <span>截止时间</span>
                  </div>
                  {chapter.jobs.map((job) => {
                    const overdue =
                      job.status.kind !== "finished" &&
                      isOverdue(job.deadline);
                    return (
                      <Link
                        key={job.job_id}
                        to={jobPath(job)}
                        className="job-row job-row--book"
                      >
                        <span className="job-name">
                          <span className="job-name-main">{job.name}</span>
                          {job.read_aloud && (
                            <span className="job-sub">含朗读</span>
                          )}
                        </span>
                        <StatusBadge kind={job.status.kind} />
                        <span className="job-score">
                          {job.score != null ? `${job.score} 分` : "—"}
                        </span>
                        <span
                          className={`job-deadline${overdue ? " job-deadline--overdue" : ""}`}
                        >
                          {formatDateTime(job.deadline)}
                        </span>
                      </Link>
                    );
                  })}
                </>
              )}
            </section>
          ))}
        </>
      )}
    </div>
  );
}
