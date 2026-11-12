import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { WordPkPreview, WordPkResult } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon, CheckIcon, RefreshIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage } from "../utils";

export function WordPk() {
  const { jobId } = useParams();
  const account = useAccount();
  const queryClient = useQueryClient();

  const [score, setScore] = useState("8");
  const [total, setTotal] = useState("10");
  const [preview, setPreview] = useState<WordPkPreview | null>(null);
  const [result, setResult] = useState<WordPkResult | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  // Drawing calls the bucket endpoint and mints the one-shot token, so it is a
  // user action rather than an automatic query: re-rendering must not burn a
  // new round.
  const draw = useMutation({
    mutationFn: (request: { score: number; total: number }) =>
      api.previewWordPk(
        account.account_id,
        jobId ?? "",
        request.score,
        request.total,
      ),
    onSuccess: (data) => {
      setPreview(data);
      setResult(null);
    },
  });

  const submit = useMutation({
    mutationFn: () => {
      // The token belongs to the preview that was shown; without it the backend
      // refuses, so a double click cannot submit twice.
      const confirmToken = preview?.confirm_token;
      if (!confirmToken) {
        throw {
          code: "confirmation_expired",
          message: "预览已过期，请重新抽取",
          retryable: false,
        };
      }
      return api.submitWordPk(confirmToken);
    },
    onSuccess: (data) => {
      setResult(data);
      setConfirmOpen(false);
      queryClient.invalidateQueries({ queryKey: ["jobs", account.account_id] });
    },
  });

  const scoreNumber = Number(score);
  const totalNumber = Number(total);
  const inputValid =
    Number.isInteger(scoreNumber) &&
    Number.isInteger(totalNumber) &&
    scoreNumber >= 0 &&
    totalNumber > 0 &&
    scoreNumber <= totalNumber;

  const correctWords = (preview?.words ?? []).filter((word) => word.correct);

  function startDraw() {
    if (!inputValid || !jobId) return;
    draw.mutate({ score: scoreNumber, total: totalNumber });
  }

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{preview?.title ?? "单词 PK"}</h1>
          <p className="page-subtitle">
            {preview
              ? `词库 ${preview.pool_size} 词 · 抽 ${preview.total} 词，答对 ${preview.score} 个`
              : "输入分数与 total，从词库随机抽词后确认提交。"}
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

      {draw.isError && (
        <div className="banner banner--danger" role="alert">
          {errorMessage(draw.error)}
        </div>
      )}

      {Boolean(jobId) && (
        <div className="toolbar">
          <div className="field">
            <label className="field-label" htmlFor="wordpk-score">
              分数（答对词数）
            </label>
            <input
              id="wordpk-score"
              className="input"
              type="number"
              min={0}
              step={1}
              value={score}
              onChange={(event) => setScore(event.target.value)}
            />
          </div>

          <div className="field">
            <label className="field-label" htmlFor="wordpk-total">
              total（本局总词数）
            </label>
            <input
              id="wordpk-total"
              className="input"
              type="number"
              min={1}
              step={1}
              value={total}
              onChange={(event) => setTotal(event.target.value)}
            />
          </div>

          <button
            type="button"
            className="button button--primary"
            onClick={startDraw}
            disabled={!inputValid || draw.isPending}
          >
            <RefreshIcon />
            {draw.isPending ? "正在抽词…" : preview ? "重新抽取" : "随机抽词"}
          </button>

          {preview && (
            <button
              type="button"
              className="button"
              onClick={() => setConfirmOpen(true)}
              disabled={submit.isPending || result?.ok === true}
            >
              <CheckIcon />
              {result?.ok ? "已提交" : "提交本局"}
            </button>
          )}
        </div>
      )}

      {!inputValid && (
        <div className="banner banner--info" role="status">
          分数需为 0 到 total 之间的整数，total 至少为 1。
        </div>
      )}

      {draw.isPending && <LoadingState label="正在抽取单词…" />}

      {preview && (
        <>
          <section className="answer-summary" aria-label="本局统计">
            <div className="stat">
              <div className="stat-label">本局总词数</div>
              <div className="stat-value">{preview.total}</div>
            </div>
            <div className="stat">
              <div className="stat-label">答对（正确）</div>
              <div className="stat-value">{preview.score}</div>
            </div>
            <div className="stat">
              <div className="stat-label">答错</div>
              <div className="stat-value">{preview.total - preview.score}</div>
            </div>
            <div className="stat">
              <div className="stat-label">词库可用</div>
              <div className="stat-value">{preview.pool_size}</div>
            </div>
          </section>

          <div className="word-grid">
            {preview.words.map((word) => (
              <article
                key={word.word_id}
                className={`word-card${word.correct ? " word-card--known" : ""}`}
              >
                <div className="word-head">
                  <span className="word-title">{word.word}</span>
                  <span className="word-phonetic">{word.phonetic}</span>
                </div>
                <div className="word-translation">{word.translation}</div>
                <span
                  className={`badge ${word.correct ? "badge--finished" : "badge--expired"}`}
                >
                  {word.correct ? "正确" : "错误"}
                </span>
              </article>
            ))}
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirmOpen}
        title="提交单词 PK"
        summary={
          <>
            <p>
              将提交 {preview?.total ?? 0} 个单词：答对{" "}
              {preview?.score ?? 0} 个，答错{" "}
              {(preview?.total ?? 0) - (preview?.score ?? 0)} 个。
            </p>
            <p className="field-hint">
              单词 PK 是真实写操作；同一作业重复提交通常返回 -13（重复作答 /
              次数用尽）。
            </p>
            {correctWords.length > 0 && (
              <ul className="summary-list">
                {correctWords.slice(0, 10).map((word) => (
                  <li key={word.word_id}>{word.word}</li>
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
