import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { SubmitResult } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, CheckIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage } from "../utils";

export function WordPk() {
  const { jobId } = useParams();
  const account = useAccount();
  const [known, setKnown] = useState<Set<string>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [result, setResult] = useState<SubmitResult | null>(null);

  const preview = useQuery({
    queryKey: ["word-pk", account.account_id, jobId],
    queryFn: () => api.getWordPk(account.account_id, jobId ?? ""),
    enabled: Boolean(jobId),
  });

  const submit = useMutation({
    mutationFn: () =>
      api.submitWordPk(
        account.account_id,
        jobId ?? "",
        (preview.data?.words ?? []).map((word) => ({
          word_id: word.word_id,
          known: known.has(word.word_id),
        })),
      ),
    onSuccess: (data) => {
      setResult(data);
      setConfirmOpen(false);
    },
  });

  function toggle(wordId: string) {
    setKnown((previous) => {
      const next = new Set(previous);
      if (next.has(wordId)) {
        next.delete(wordId);
      } else {
        next.add(wordId);
      }
      return next;
    });
  }

  const words = preview.data?.words ?? [];

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{preview.data?.title ?? "单词 PK"}</h1>
          <p className="page-subtitle">
            {preview.data
              ? `词桶 ${preview.data.bucket_id} · 共 ${words.length} 个单词`
              : "预习单词后标记掌握情况。"}
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

      {preview.isPending && Boolean(jobId) && (
        <LoadingState label="正在加载单词…" />
      )}

      {preview.isError && (
        <ErrorState error={preview.error} onRetry={() => preview.refetch()} />
      )}

      {result && (
        <div className="banner banner--success" role="status">
          <strong>{result.ok ? "提交成功" : "提交失败"}</strong>
          <span>{result.message}</span>
        </div>
      )}

      {submit.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(submit.error)}
        </div>
      )}

      {preview.isSuccess && words.length === 0 && (
        <EmptyState
          title="暂无单词"
          description="该单词 PK 任务还没有同步到词表。"
        />
      )}

      {preview.isSuccess && words.length > 0 && (
        <>
          <section className="scoreboard" aria-label="比分">
            <div className="score-side">
              <div className="score-number">{preview.data?.my_score ?? "—"}</div>
              <div className="score-label">我的得分</div>
            </div>
            <span className="vs">对</span>
            <div className="score-side">
              <div className="score-number">
                {preview.data?.opponent_score ?? "—"}
              </div>
              <div className="score-label">对手得分</div>
            </div>
            <div className="toolbar-spacer" />
            <button
              type="button"
              className="button button--primary"
              onClick={() => setConfirmOpen(true)}
              disabled={submit.isPending || result?.ok === true}
            >
              <CheckIcon />
              {result?.ok ? "已提交" : "提交掌握状态"}
            </button>
          </section>

          <div className="word-grid">
            {words.map((word) => {
              const checked = known.has(word.word_id);
              return (
                <article
                  key={word.word_id}
                  className={`word-card${checked ? " word-card--known" : ""}`}
                >
                  <div className="word-head">
                    <span className="word-title">{word.word}</span>
                    <span className="word-phonetic">{word.phonetic}</span>
                  </div>
                  <div className="word-translation">{word.translation}</div>
                  <label className="check" htmlFor={`known-${word.word_id}`}>
                    <input
                      id={`known-${word.word_id}`}
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggle(word.word_id)}
                    />
                    已掌握
                  </label>
                </article>
              );
            })}
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirmOpen}
        title="提交单词掌握状态"
        summary={
          <>
            <p>
              将提交 {words.length} 个单词，其中 {known.size} 个标记为已掌握。
            </p>
            {known.size > 0 && (
              <ul className="summary-list">
                {words
                  .filter((word) => known.has(word.word_id))
                  .slice(0, 10)
                  .map((word) => (
                    <li key={word.word_id}>
                      {word.word} — {word.translation}
                    </li>
                  ))}
              </ul>
            )}
          </>
        }
        confirmLabel="确认提交"
        busy={submit.isPending}
        onConfirm={() => submit.mutate()}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}
