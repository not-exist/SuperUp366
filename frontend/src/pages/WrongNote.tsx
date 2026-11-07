import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { CleanupPreview } from "../api/types";
import { ClassChip } from "../components/ClassChip";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  TrashIcon,
} from "../components/Icons";
import {
  ProgressList,
  type ProgressEntry,
} from "../components/ProgressList";
import { useAccount } from "../hooks/account";
import { errorMessage, formatDateTime } from "../utils";

const PAGE_SIZE = 20;

export function WrongNote() {
  const account = useAccount();
  const queryClient = useQueryClient();

  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [progress, setProgress] = useState<ProgressEntry[]>([]);
  const [running, setRunning] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const allRef = useRef<HTMLInputElement>(null);

  const notes = useQuery({
    queryKey: ["wrong-notes", account.account_id, page],
    queryFn: () => api.listWrongNotes(account.account_id, page, PAGE_SIZE),
  });

  const items = notes.data?.items ?? [];
  const total = notes.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const allSelected =
    items.length > 0 && items.every((note) => selected.has(note.question_id));
  const someSelected = items.some((note) => selected.has(note.question_id));

  useEffect(() => {
    setSelected(new Set());
  }, [page]);

  useEffect(() => {
    if (allRef.current) {
      allRef.current.indeterminate = someSelected && !allSelected;
    }
  }, [someSelected, allSelected]);

  useEffect(() => {
    if (page > 1 && notes.isSuccess && items.length === 0) {
      setPage((current) => Math.max(1, current - 1));
    }
  }, [page, notes.isSuccess, items.length]);

  const previewMutation = useMutation({
    mutationFn: () =>
      api.previewWrongNoteCleanup(
        account.account_id,
        Array.from(selected),
      ),
    onSuccess: (data) => {
      setPreview(data);
      setProgress([]);
      setSummary(null);
      setConfirmOpen(true);
    },
  });

  function toggle(questionId: string) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(questionId)) {
        next.delete(questionId);
      } else {
        next.add(questionId);
      }
      return next;
    });
  }

  function toggleAll() {
    setSelected(
      allSelected
        ? new Set()
        : new Set(items.map((note) => note.question_id)),
    );
  }

  async function runCleanup() {
    if (!preview) return;
    setRunning(true);
    setSummary(null);
    setProgress(
      preview.items.map((item) => ({
        id: item.question_id,
        label: item.title,
        status: "pending",
      })),
    );

    let done = 0;
    let failed = 0;

    for (const item of preview.items) {
      setProgress((previous) =>
        previous.map((entry) =>
          entry.id === item.question_id
            ? { ...entry, status: "running" }
            : entry,
        ),
      );
      try {
        await api.cleanupWrongNote(account.account_id, item.question_id);
        done += 1;
        setProgress((previous) =>
          previous.map((entry) =>
            entry.id === item.question_id
              ? { ...entry, status: "done", detail: "已清理" }
              : entry,
          ),
        );
      } catch (error) {
        failed += 1;
        setProgress((previous) =>
          previous.map((entry) =>
            entry.id === item.question_id
              ? { ...entry, status: "failed", detail: errorMessage(error) }
              : entry,
          ),
        );
      }
    }

    setRunning(false);
    setSummary(
      `已清理 ${done} 题${failed > 0 ? `，${failed} 题失败` : ""}。`,
    );
    setSelected(new Set());
    await queryClient.invalidateQueries({
      queryKey: ["wrong-notes", account.account_id],
    });
  }

  function closeDialog() {
    if (running) return;
    setConfirmOpen(false);
    setPreview(null);
    setProgress([]);
    setSummary(null);
    previewMutation.reset();
  }

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">错题本</h1>
          <p className="page-subtitle">
            勾选题目后一键清理。清理前会展示将提交的内容，清理过程逐题显示进度。
          </p>
        </div>
      </header>

      {notes.isPending && <LoadingState label="正在加载错题…" />}

      {notes.isError && (
        <ErrorState error={notes.error} onRetry={() => notes.refetch()} />
      )}

      {notes.isSuccess && total === 0 && (
        <EmptyState
          title="错题本为空"
          description="当前账号没有需要清理的错题。"
        />
      )}

      {notes.isSuccess && total > 0 && (
        <>
          <div className="toolbar">
            <label className="check" htmlFor="wrong-select-all">
              <input
                id="wrong-select-all"
                ref={allRef}
                type="checkbox"
                checked={allSelected}
                onChange={toggleAll}
              />
              全选本页
            </label>
            <span className="field-hint">已选 {selected.size} 题</span>
            <div className="toolbar-spacer" />
            <button
              type="button"
              className="button button--danger"
              onClick={() => previewMutation.mutate()}
              disabled={selected.size === 0 || previewMutation.isPending}
            >
              <TrashIcon />
              {previewMutation.isPending
                ? "正在生成预览…"
                : `一键清理${selected.size > 0 ? `（${selected.size}）` : ""}`}
            </button>
          </div>

          {previewMutation.isError && (
            <div className="banner banner--danger" role="alert">
              无法生成清理预览：{errorMessage(previewMutation.error)}
            </div>
          )}

          <div className="wrong-list">
            {items.map((note) => {
              const checked = selected.has(note.question_id);
              return (
                <div
                  key={note.question_id}
                  className={`wrong-row${checked ? " wrong-row--selected" : ""}`}
                >
                  <div className="wrong-check">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(note.question_id)}
                      aria-label={`选择错题：${note.title}，${note.stem}`}
                    />
                  </div>
                  <div>
                    <div className="wrong-meta">
                      <ClassChip label={note.class_name} />
                      <span>{note.title}</span>
                      <span aria-hidden="true">·</span>
                      <span>{note.question_type}</span>
                      <span aria-hidden="true">·</span>
                      <span>{formatDateTime(note.added_at)}</span>
                    </div>
                    <p className="wrong-stem">{note.stem}</p>
                    <div className="wrong-answer">
                      你的答案：{note.student_answer || "（空）"} · 正确答案：
                      {note.correct_answer}
                    </div>
                  </div>
                  <span className="badge badge--expired">错题</span>
                </div>
              );
            })}
          </div>

          <div className="pagination">
            <span>
              第 {page} / {totalPages} 页 · 共 {total} 题
            </span>
            <button
              type="button"
              className="button button--sm"
              onClick={() => setPage((current) => Math.max(1, current - 1))}
              disabled={page <= 1}
            >
              <ChevronLeftIcon />
              上一页
            </button>
            <button
              type="button"
              className="button button--sm"
              onClick={() =>
                setPage((current) => Math.min(totalPages, current + 1))
              }
              disabled={page >= totalPages}
            >
              下一页
              <ChevronRightIcon />
            </button>
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirmOpen}
        title={summary ? "清理完成" : "确认一键清理"}
        summary={
          summary ? (
            <>
              <p role="status">{summary}</p>
              <ProgressList items={progress} />
            </>
          ) : preview ? (
            <>
              <p>
                将清理 {preview.total} 题，预计约 {preview.estimated_seconds}{" "}
                秒。
              </p>
              <p className="field-hint">
                {preview.batch_mode === "per_page"
                  ? "服务端按页提交，进度以页为单位。"
                  : "服务端逐题提交，下面逐题显示进度。"}
              </p>
              <ul className="summary-list">
                {preview.items.slice(0, 12).map((item) => (
                  <li key={item.question_id}>{item.title}</li>
                ))}
                {preview.items.length > 12 && (
                  <li>以及其余 {preview.items.length - 12} 题</li>
                )}
              </ul>
              {progress.length > 0 && <ProgressList items={progress} />}
            </>
          ) : (
            <p>正在生成清理预览…</p>
          )
        }
        confirmLabel={summary ? "关闭" : "确认清理"}
        tone={summary ? "default" : "danger"}
        busy={running}
        hideCancel={Boolean(summary)}
        onConfirm={() => {
          if (summary) {
            closeDialog();
          } else {
            void runCleanup();
          }
        }}
        onCancel={closeDialog}
      />
    </div>
  );
}
