import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { api } from "../api/commands";
import type { JobKind, JobRow, JobStatusKind } from "../api/types";
import { ClassChip } from "../components/ClassChip";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import {
  RefreshIcon,
  SearchIcon,
} from "../components/Icons";
import {
  STATUS_LABELS,
  STATUS_OPTIONS,
  StatusBadge,
} from "../components/StatusBadge";
import { useAccount } from "../hooks/account";
import {
  formatDateTime,
  isOverdue,
  jobPath,
} from "../utils";

type StatusFilter = "all" | JobStatusKind;
/** Read-aloud is a paper property, not a `JobKind`, but users filter it as one. */
type KindFilter = "all" | JobKind | "read_aloud";

const KIND_LABELS: Record<JobKind, string> = {
  ordinary: "普通作业",
  word_pk: "单词 PK",
  applied_writing: "应用文",
  dictation: "默写线索",
  unknown: "类型未知",
};

const READ_ALOUD_LABEL = "跟读作业";

function kindLabel(job: JobRow): string | null {
  if (job.kind === "word_pk") return "单词 PK";
  if (job.kind === "applied_writing") return "应用文线索";
  if (job.kind === "dictation") return "默写线索 · 协议待确认";
  if (job.read_aloud) return "含朗读";
  return null;
}

export function Jobs() {
  const account = useAccount();
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [kind, setKind] = useState<KindFilter>("all");
  const scrollRef = useRef<HTMLDivElement>(null);

  const jobs = useQuery({
    queryKey: ["jobs", account.account_id],
    queryFn: () => api.listJobs(account.account_id),
  });

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return (jobs.data ?? []).filter((job) => {
      if (status !== "all" && job.status.kind !== status) return false;
      if (kind === "read_aloud" && !job.read_aloud) return false;
      if (kind !== "all" && kind !== "read_aloud" && job.kind !== kind) {
        return false;
      }
      if (!needle) return true;
      return (
        job.name.toLowerCase().includes(needle) ||
        job.class_name.toLowerCase().includes(needle) ||
        job.course_name.toLowerCase().includes(needle)
      );
    });
  }, [jobs.data, search, status, kind]);

  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 65,
    overscan: 8,
  });

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 });
  }, [search, status, kind]);

  const hasFilters = search.trim() !== "" || status !== "all" || kind !== "all";

  function resetFilters() {
    setSearch("");
    setStatus("all");
    setKind("all");
  }

  return (
    <div className="page page--fill" data-testid="jobs-page">
      <header className="page-header">
        <div>
          <h1 className="page-title">作业</h1>
          <p className="page-subtitle" data-testid="jobs-summary">
            所有班级合并为一个列表，保持官方顺序。共 {jobs.data?.length ?? 0} 项
            {hasFilters ? `，筛选后 ${filtered.length} 项` : ""}。
          </p>
        </div>
        <div className="page-actions">
          <button
            type="button"
            className="button"
            onClick={() => jobs.refetch()}
            disabled={jobs.isFetching}
          >
            <RefreshIcon />
            {jobs.isFetching ? "刷新中…" : "刷新"}
          </button>
        </div>
      </header>

      <div className="toolbar">
        <div className="field field--search">
          <label className="field-label" htmlFor="job-search">
            搜索
          </label>
          <div className="search-wrap">
            <SearchIcon />
            <input
              id="job-search"
              className="input"
              type="search"
              placeholder="作业名或班级"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </div>
        </div>

        <div className="field">
          <label className="field-label" htmlFor="job-status">
            状态
          </label>
          <select
            id="job-status"
            className="select"
            value={status}
            onChange={(event) => setStatus(event.target.value as StatusFilter)}
          >
            <option value="all">全部状态</option>
            {STATUS_OPTIONS.map((item) => (
              <option key={item} value={item}>
                {STATUS_LABELS[item]}
              </option>
            ))}
          </select>
        </div>

        <div className="field">
          <label className="field-label" htmlFor="job-kind">
            类型
          </label>
          <select
            id="job-kind"
            className="select"
            value={kind}
            onChange={(event) => setKind(event.target.value as KindFilter)}
          >
            <option value="all">全部类型</option>
            <option value="ordinary">{KIND_LABELS.ordinary}</option>
            <option value="word_pk">{KIND_LABELS.word_pk}</option>
            <option value="applied_writing">{KIND_LABELS.applied_writing}</option>
            <option value="dictation">{KIND_LABELS.dictation}</option>
            <option value="read_aloud">{READ_ALOUD_LABEL}</option>
            <option value="unknown">{KIND_LABELS.unknown}</option>
          </select>
        </div>
      </div>

      <div className="job-list-head" aria-hidden="true">
        <span>作业</span>
        <span>班级</span>
        <span>状态</span>
        <span>分数</span>
        <span>截止时间</span>
      </div>

      {jobs.isPending && <LoadingState label="正在加载作业…" />}

      {jobs.isError && (
        <ErrorState error={jobs.error} onRetry={() => jobs.refetch()} />
      )}

      {jobs.isSuccess && filtered.length === 0 && (
        <EmptyState
          title={hasFilters ? "没有匹配的作业" : "暂无作业"}
          description={
            hasFilters
              ? "调整搜索词或筛选条件后再试。"
              : "当前账号还没有下发作业。"
          }
          action={
            hasFilters ? (
              <button type="button" className="button" onClick={resetFilters}>
                清除筛选
              </button>
            ) : undefined
          }
        />
      )}

      {jobs.isSuccess && filtered.length > 0 && (
        <div
          ref={scrollRef}
          className="job-list-scroll"
          data-testid="job-list"
          role="list"
          aria-label="作业列表"
        >
          <div
            style={{
              height: virtualizer.getTotalSize(),
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const job = filtered[virtualRow.index];
              const overdue =
                job.status.kind !== "finished" && isOverdue(job.deadline);
              const label = kindLabel(job);
              return (
                <div
                  key={`${job.course_id}:${job.job_id}`}
                  role="listitem"
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: virtualRow.size,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <Link
                    to={jobPath(job)}
                    data-testid={`job-row-${job.job_id}`}
                    className="job-row"
                    style={{ height: "100%" }}
                  >
                    <span className="job-name">
                      <span className="job-name-main">{job.name}</span>
                      {label && <span className="job-sub">{label}</span>}
                    </span>
                    <ClassChip label={job.class_name} />
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
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
