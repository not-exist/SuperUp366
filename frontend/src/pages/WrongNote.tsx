import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type {
  CleanupPreview,
  CleanupRun,
  WrongNote,
} from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { TrashIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage } from "../utils";

export function WrongNote() {
  const account = useAccount();

  const [subjectId, setSubjectId] = useState<string>("");
  const [knowledgeId, setKnowledgeId] = useState<string>("");
  const [qtypeId, setQtypeId] = useState<string>("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [run, setRun] = useState<CleanupRun | null>(null);

  const subjects = useQuery({
    queryKey: ["wrong-subjects", account.account_id],
    queryFn: () => api.listWrongSubjects(account.account_id),
  });

  const knowledge = useQuery({
    queryKey: ["wrong-knowledge", account.account_id, subjectId],
    queryFn: () => api.listWrongKnowledge(account.account_id, subjectId),
    enabled: Boolean(subjectId),
  });

  const pending = useQuery({
    queryKey: [
      "wrong-pending",
      account.account_id,
      subjectId,
      knowledgeId,
      qtypeId,
    ],
    queryFn: () =>
      api.listWrongPending(
        account.account_id,
        subjectId,
        "",
        knowledgeId,
        qtypeId,
      ),
    enabled: Boolean(subjectId) && Boolean(knowledgeId) && Boolean(qtypeId),
  });

  const items: WrongNote[] = pending.data ?? [];
  const allSelected =
    items.length > 0 &&
    items.every((note) => selected.has(note.question_id));

  const previewMutation = useMutation({
    mutationFn: () =>
      api.previewWrongNoteCleanup(
        account.account_id,
        Array.from(selected),
        {
          subject_id: subjectId,
          book_id: "",
          knowledge_id: knowledgeId,
          qtype_id: qtypeId,
        },
      ),
    onSuccess: (data) => {
      setPreview(data);
      setRun(null);
      setConfirmOpen(true);
    },
  });

  const cleanupMutation = useMutation({
    mutationFn: () => api.cleanupWrongNotes(preview?.confirm_token ?? ""),
    onSuccess: (data) => {
      setRun(data);
      setConfirmOpen(false);
      setPreview(null);
      setSelected(new Set());
      // The counts on both pickers change with every cleared question.
      void pending.refetch();
      void knowledge.refetch();
      void subjects.refetch();
    },
    onError: () => {
      // The token is single-use: the server consumed it even when the run
      // failed, so the dialog must not offer the same token again.
      setConfirmOpen(false);
      setPreview(null);
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

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">错题本</h1>
          <p className="page-subtitle">
            选择学科与知识点，会加载该知识点下的全部未清理错题。
          </p>
        </div>
      </header>

      {subjects.isError && (
        <ErrorState error={subjects.error} onRetry={() => subjects.refetch()} />
      )}

      <section className="filter-row">
        <label className="field">
          <span className="field-label">学科</span>
          <select
            className="input"
            value={subjectId}
            onChange={(event) => {
              setSubjectId(event.target.value);
              setKnowledgeId("");
              setQtypeId("");
              setSelected(new Set());
            }}
          >
            <option value="">请选择学科</option>
            {(subjects.data ?? []).map((subject) => (
              <option key={subject.subject_id} value={subject.subject_id}>
                {subject.subject_name}（未清理 {subject.wrong_count}）
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field-label">知识点</span>
          <select
            className="input"
            value={knowledgeId ? `${knowledgeId}|${qtypeId}` : ""}
            onChange={(event) => {
              const [nextKnowledgeId, nextQtypeId] = event.target.value.split("|");
              setKnowledgeId(nextKnowledgeId ?? "");
              setQtypeId(nextQtypeId ?? "");
              setSelected(new Set());
            }}
            disabled={!subjectId}
          >
            <option value="">请选择知识点</option>
            {(knowledge.data ?? []).map((point) => (
              <option
                key={`${point.knowledge_id}|${point.qtype_id}`}
                value={`${point.knowledge_id}|${point.qtype_id}`}
              >
                {point.knowledge_name}（{point.qtype_name}，{point.wrong_count}）
              </option>
            ))}
          </select>
        </label>
      </section>

      {subjects.isPending && <LoadingState label="正在加载学科…" />}
      {knowledge.isPending && Boolean(subjectId) && (
        <LoadingState label="正在加载知识点…" />
      )}

      {pending.isError && (
        <ErrorState error={pending.error} onRetry={() => pending.refetch()} />
      )}

      {!subjectId && !subjects.isPending && (
        <EmptyState
          title="先选择学科"
          description="选择学科后才能看到知识点与未清理的错题。"
        />
      )}

      {subjectId && !knowledgeId && !knowledge.isPending && (
        <EmptyState
          title="再选择知识点"
          description="未清理的错题按知识点分组。"
        />
      )}

      {pending.isSuccess && items.length === 0 && Boolean(knowledgeId) && (
        <EmptyState
          title="没有未清理的错题"
          description="该知识点下暂无需要清理的题目。"
        />
      )}

      {pending.isPending && Boolean(knowledgeId) && (
        <LoadingState label="正在加载错题…" />
      )}

      {items.length > 0 && (
        <>
          <div className="overall-row">
            <label className="checkbox">
              <input
                type="checkbox"
                checked={allSelected}
                onChange={toggleAll}
              />
              <span>全选（{items.length} 题）</span>
            </label>
            <button
              type="button"
              className="button button--primary"
              disabled={selected.size === 0 || previewMutation.isPending}
              onClick={() => previewMutation.mutate()}
            >
              <TrashIcon />
              清理所选（{selected.size}）
            </button>
          </div>

          <ul className="wrong-list">
            {items.map((note) => {
              const checked = selected.has(note.question_id);
              return (
                <li key={note.question_id} className="wrong-item">
                  <label className="checkbox">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(note.question_id)}
                      aria-label={`选择错题：${note.title}，${note.stem}`}
                    />
                  </label>
                  <div className="wrong-body">
                    <div className="wrong-meta">
                      <strong>{note.title}</strong>
                      <span className="badge">{note.question_type}</span>
                    </div>
                    <p className="wrong-stem">{note.stem}</p>
                    <p className="wrong-answer">
                      正确答案：{note.correct_answer || "（未提供）"}
                      {note.student_answer
                        ? ` · 你的答案：${note.student_answer}`
                        : ""}
                    </p>
                  </div>
                </li>
              );
            })}
          </ul>
        </>
      )}

      {previewMutation.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(previewMutation.error)}
        </div>
      )}

      {run && (
        <div
          className={`banner ${run.ok ? "banner--success" : "banner--info"}`}
          role="status"
        >
          <strong>{run.ok ? "清理完成" : "部分完成"}</strong>
          <span>{run.message}</span>
        </div>
      )}

      <ConfirmDialog
        open={confirmOpen && preview !== null}
        title="清理所选错题"
        onCancel={() => {
          setConfirmOpen(false);
          setPreview(null);
        }}
        onConfirm={() => cleanupMutation.mutate()}
        confirmLabel="确认清理"
        busy={cleanupMutation.isPending}
        summary={
          <>
            <p>
              将逐题提交 {preview?.items.length ?? 0} 道错题，提交成功后这些题目会从
              错题本移除。失败可单独重试。
            </p>
            <ul className="summary-list">
              {(preview?.items ?? []).slice(0, 8).map((item) => (
                <li key={item.question_id}>{item.title}</li>
              ))}
            </ul>
          </>
        }
      />

      {cleanupMutation.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(cleanupMutation.error)}
        </div>
      )}
    </div>
  );
}
