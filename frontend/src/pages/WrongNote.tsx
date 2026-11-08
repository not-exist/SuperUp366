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
      void pending.refetch();
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
            选择学科与知识点，勾选未清理的错题后一键清理。
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
              <span>全选本页（{items.length} 题）</span>
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
        <div className="banner banner--warning" role="status">
          <strong>{run.ok ? "已完成" : "未执行"}</strong>
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
        confirmLabel={preview?.granularity_confirmed ? "确认清理" : "生成预览"}
        busy={cleanupMutation.isPending}
        summary={
          <>
            <p>
              将处理 {preview?.items.length ?? 0} 道错题。
              {preview?.granularity_confirmed
                ? "逐题提交，失败可单独重试。"
                : "当前未对真实账号验证服务端是否支持逐题提交，本次只会生成预览，不会发送任何请求。"}
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
