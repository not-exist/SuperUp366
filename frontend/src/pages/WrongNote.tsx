import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { CorrectionDraw, CorrectionSubmit } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ErrorState } from "../components/ErrorState";
import { CheckIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage } from "../utils";

export function WrongNote() {
  const account = useAccount();

  const [correctionCount, setCorrectionCount] = useState(10);
  const [correctionGroup, setCorrectionGroup] = useState("");
  const [drawn, setDrawn] = useState<CorrectionDraw | null>(null);
  const [correctionConfirmOpen, setCorrectionConfirmOpen] = useState(false);
  const [correctionRun, setCorrectionRun] = useState<CorrectionSubmit | null>(
    null,
  );

  const correction = useQuery({
    queryKey: ["correction-overview", account.account_id],
    queryFn: () => api.getCorrectionOverview(account.account_id),
    // The remaining count changes as soon as a correction is submitted, so it
    // must not be served from the 15 s page cache.
    staleTime: 0,
    refetchOnMount: "always",
  });

  const drawCorrectionMutation = useMutation({
    mutationFn: () =>
      api.drawCorrection(
        account.account_id,
        correctionCount,
        correctionGroup || undefined,
      ),
    onSuccess: (data) => {
      setDrawn(data);
      setCorrectionRun(null);
      if (data.confirm_token) {
        setCorrectionConfirmOpen(true);
      }
    },
  });

  const submitCorrectionMutation = useMutation({
    mutationFn: () => api.submitCorrection(drawn?.confirm_token ?? ""),
    onSuccess: (data) => {
      setCorrectionRun(data);
      setCorrectionConfirmOpen(false);
      setDrawn(null);
      void correction.refetch();
    },
    onError: () => {
      // The token is single-use, even when the submit failed.
      setCorrectionConfirmOpen(false);
      setDrawn(null);
    },
  });

  const correctionStatus = useQuery({
    queryKey: [
      "correction-status",
      account.account_id,
      correctionRun?.batch_id ?? "",
    ],
    queryFn: () =>
      api.getCorrectionStatus(
        account.account_id,
        correctionRun?.batch_id ?? "",
      ),
    enabled: Boolean(correctionRun?.batch_id),
    refetchInterval: (query) =>
      query.state.data?.status === 1 ? 4000 : false,
  });

  const remainingEntries = (correction.data?.groups ?? []).filter(
    (group) => group.remaining_count > 0,
  );
  const maxCorrectionCount = Math.min(
    50,
    Math.max(1, correction.data?.remaining_count ?? 1),
  );

  return (
    <div className="page" data-testid="wrongnote-page">
      <header className="page-header">
        <div>
          <h1 className="page-title">错题本</h1>
          <p className="page-subtitle">
            按题型抽题、预览标准答案，再一次性提交订正。每道题只能订正一次。
          </p>
        </div>
      </header>

      <section className="correction-panel">
        <div className="correction-head">
          <div>
            <h2 className="correction-title">一键订正</h2>
            {correction.isSuccess ? (
              <div className="correction-stats">
                <strong data-testid="correction-remaining">
                  未清理 {correction.data.remaining_count} 题
                </strong>
                <span>已订正 {correction.data.corrected_count} 题</span>
                <span>今日 {correction.data.today_corrected_count} 题</span>
                <button
                  type="button"
                  className="button button--sm button--ghost"
                  onClick={() => correction.refetch()}
                  disabled={correction.isFetching}
                >
                  刷新数量
                </button>
              </div>
            ) : correction.isError ? (
              <p className="field-hint" role="alert">
                错题本总览读取失败：{errorMessage(correction.error)}
              </p>
            ) : (
              <p className="page-subtitle">正在读取错题本总览…</p>
            )}
          </div>
          <div className="correction-controls">
            <label className="field">
              <span className="field-label">题量</span>
              <input
                className="input"
                type="number"
                min={1}
                max={maxCorrectionCount}
                data-testid="correction-count"
                value={correctionCount}
                onChange={(event) =>
                  setCorrectionCount(
                    Math.min(
                      maxCorrectionCount,
                      Math.max(1, Number(event.target.value) || 1),
                    ),
                  )
                }
              />
            </label>
            <label className="field">
              <span className="field-label">题型范围</span>
              <select
                className="input"
                value={correctionGroup}
                onChange={(event) => setCorrectionGroup(event.target.value)}
              >
                <option value="">全部题型</option>
                {remainingEntries.map((group) => (
                  <option key={group.group_key} value={group.group_key}>
                    {group.qtype_name || "未命名题型"}
                    {group.knowledge_name &&
                    !["--", "－", "-", ""].includes(
                      group.knowledge_name.trim(),
                    )
                      ? `（${group.knowledge_name}）`
                      : ""}
                    （剩余 {group.remaining_count}）
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className="button button--primary"
              data-testid="correction-draw"
              disabled={drawCorrectionMutation.isPending}
              onClick={() => drawCorrectionMutation.mutate()}
            >
              <CheckIcon />
              抽题预览
            </button>
          </div>
        </div>

        {correction.isError && (
          <ErrorState
            error={correction.error}
            onRetry={() => correction.refetch()}
          />
        )}

        {drawCorrectionMutation.isError && (
          <div className="banner banner--danger" role="alert">
            {errorMessage(drawCorrectionMutation.error)}
          </div>
        )}

        {correctionRun && (
          <div
            className="banner banner--success"
            role="status"
            data-testid="correction-result"
          >
            <strong>订正已提交</strong>
            <span>{correctionRun.message}</span>
            {correctionStatus.data && (
              <span>
                批改状态：
                {correctionStatus.data.status === 1
                  ? "正在批改"
                  : correctionStatus.data.status === 2
                    ? "批改完成"
                    : "尚未提交"}
              </span>
            )}
            {correctionRun.note_message && (
              <span>{correctionRun.note_message}</span>
            )}
          </div>
        )}

        {submitCorrectionMutation.isError && (
          <div className="banner banner--danger" role="alert">
            {errorMessage(submitCorrectionMutation.error)}
          </div>
        )}

        {drawn && drawn.confirm_token === null && (
          <div className="banner banner--info" role="status">
            <strong>没有需要订正的题目</strong>
            <span>当前范围的错题已经全部订正。</span>
          </div>
        )}

        {drawn && drawn.questions.length > 0 && (
          <div className="correction-preview" data-testid="correction-preview">
            <div className="overall-row">
              <span>
                本次抽到 {drawn.questions.length} 题，共 {drawn.total_score} 分
              </span>
              {drawn.confirm_token && (
                <button
                  type="button"
                  className="button button--primary"
                  data-testid="correction-submit"
                  onClick={() => setCorrectionConfirmOpen(true)}
                >
                  提交这份订正
                </button>
              )}
            </div>
            <ul className="wrong-list">
              {drawn.questions.map((question) => (
                <li key={question.question_id} className="wrong-item">
                  <div className="wrong-body">
                    <div className="wrong-meta">
                      <strong>{question.type_name || "题目"}</strong>
                      <span className="badge">{question.score} 分</span>
                    </div>
                    <p className="wrong-stem">{question.stem}</p>
                    <p className="wrong-answer">
                      正确答案：{question.answer || "（未提供）"}
                    </p>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>

      <ConfirmDialog
        open={
          correctionConfirmOpen &&
          drawn !== null &&
          drawn.confirm_token !== null
        }
        title="提交订正"
        onCancel={() => setCorrectionConfirmOpen(false)}
        onConfirm={() => submitCorrectionMutation.mutate()}
        confirmLabel="确认提交"
        busy={submitCorrectionMutation.isPending}
        summary={
          <>
            <p>
              将以服务端给出的标准答案订正 {drawn?.questions.length ?? 0} 道题，
              共 {drawn?.total_score ?? 0} 分。每道题只能订正一次，提交后不可撤销。
            </p>
            <ul className="summary-list">
              {(drawn?.questions ?? []).slice(0, 8).map((question) => (
                <li key={question.question_id}>{question.stem}</li>
              ))}
            </ul>
          </>
        }
      />
    </div>
  );
}
