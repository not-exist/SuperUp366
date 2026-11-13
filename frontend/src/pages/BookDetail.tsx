import { useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { BookChapter } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon } from "../components/Icons";
import { StatusBadge } from "../components/StatusBadge";
import { useAccount } from "../hooks/account";
import { formatDateTime, isOverdue, percent } from "../utils";

/** Rows rendered per chapter before the "show more" button appears. */
const CHAPTER_PAGE_SIZE = 100;

export function BookDetail() {
  const { bookId } = useParams();
  const [params] = useSearchParams();
  const account = useAccount();
  // The shelf carries the real `bookType`; a type-2 book fails `book_info` if
  // it is requested as type 1. A direct URL without it falls back to 1.
  const bookType = Number(params.get("type")) || 1;
  // Some class-assigned books only return their chapters when the assigning
  // course is included in the tree request.
  const courseId = params.get("course");

  const detail = useQuery({
    queryKey: ["book", account.account_id, bookId, bookType, courseId],
    queryFn: () =>
      api.getBook(account.account_id, bookId ?? "", bookType, courseId),
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
                {detail.data.is_2025_reader ? "2025 阅读器" : "同步练习"}
                {detail.data.teacher_name
                  ? ` · ${detail.data.teacher_name}`
                  : ""}
              </p>
              <p className="book-meta">
                {detail.data.total > 0
                  ? `已完成 ${detail.data.progress}/${detail.data.total}`
                  : "暂无作业"}
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

          {detail.data.chapters.length === 0 ? (
            <EmptyState
              title="暂无章节"
              description="这本书还没有可显示的章节。"
            />
          ) : (
            detail.data.chapters.map((chapter) => (
              <ChapterSection key={chapter.chapter_id} chapter={chapter} />
            ))
          )}
        </>
      )}
    </div>
  );
}

/**
 * One top-level chapter. Rows are not links: a book page's task id is not a
 * job id, so the answer route would fail to find it in the job cache.
 */
function ChapterSection({ chapter }: { chapter: BookChapter }) {
  const [limit, setLimit] = useState(CHAPTER_PAGE_SIZE);
  const jobs = chapter.jobs ?? [];
  const visible = jobs.slice(0, limit);

  return (
    <section className="chapter">
      <h3 className="chapter-title">{chapter.title}</h3>
      {jobs.length === 0 ? (
        <p className="field-hint">本章暂无作业。</p>
      ) : (
        <>
          <div className="job-list-head job-list-head--book" aria-hidden="true">
            <span>作业</span>
            <span>状态</span>
            <span>分数</span>
            <span>截止时间</span>
          </div>
          {visible.map((job) => {
            const overdue =
              job.status.kind !== "finished" && isOverdue(job.deadline);
            return (
              <div key={job.job_id} className="job-row job-row--book">
                <span className="job-name">
                  <span className="job-name-main">{job.name}</span>
                  {job.read_aloud && <span className="job-sub">含朗读</span>}
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
              </div>
            );
          })}
          {jobs.length > limit && (
            <button
              type="button"
              className="button button--ghost button--sm chapter-more"
              onClick={() => setLimit((current) => current + CHAPTER_PAGE_SIZE)}
            >
              显示更多（还有 {jobs.length - limit} 项）
            </button>
          )}
        </>
      )}
    </section>
  );
}
