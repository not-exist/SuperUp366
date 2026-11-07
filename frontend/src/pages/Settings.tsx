import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { Settings } from "../api/types";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { CheckIcon, TrashIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";
import { errorMessage } from "../utils";

function clamp(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min;
  return Math.min(max, Math.max(min, Math.round(value)));
}

export function SettingsPage() {
  const account = useAccount();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(false);
  const [cacheOpen, setCacheOpen] = useState(false);
  const [cacheMessage, setCacheMessage] = useState<string | null>(null);

  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
  });

  useEffect(() => {
    if (settings.data) {
      setForm(settings.data);
    }
  }, [settings.data]);

  const update = useMutation({
    mutationFn: (next: Settings) => api.updateSettings(next),
    onSuccess: (data) => {
      setForm(data);
      setSaved(true);
      queryClient.setQueryData(["settings"], data);
    },
  });

  const clearCache = useMutation({
    mutationFn: () => api.clearCache(),
    onSuccess: () => {
      setCacheOpen(false);
      setCacheMessage("本地缓存已清理。");
    },
  });

  function patch<K extends keyof Settings>(key: K, value: Settings[K]) {
    setForm((previous) =>
      previous ? { ...previous, [key]: value } : previous,
    );
    setSaved(false);
  }

  if (settings.isPending) {
    return (
      <div className="page">
        <LoadingState label="正在加载设置…" />
      </div>
    );
  }

  if (settings.isError) {
    return (
      <div className="page">
        <ErrorState error={settings.error} onRetry={() => settings.refetch()} />
      </div>
    );
  }

  if (!form) {
    return (
      <div className="page">
        <EmptySettings />
      </div>
    );
  }

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">设置</h1>
          <p className="page-subtitle">
            请求参数、朗读提交模式与本地缓存。修改后点击保存生效。
          </p>
        </div>
      </header>

      <section className="card settings-card" aria-labelledby="account-heading">
        <h2 className="section-title" id="account-heading">
          当前账号
        </h2>
        <div className="form-row">
          <div>
            <div className="stat-label">姓名</div>
            <div>{account.display_name}</div>
          </div>
          <div>
            <div className="stat-label">账号</div>
            <div>{account.username}</div>
          </div>
          <div>
            <div className="stat-label">学校</div>
            <div>{account.school_name}</div>
          </div>
          <div>
            <div className="stat-label">会话</div>
            <div>
              <span
                className={`badge ${account.session_valid ? "badge--finished" : "badge--expired"}`}
              >
                {account.session_valid ? "有效" : "需要重新登录"}
              </span>
            </div>
          </div>
        </div>
      </section>

      <form
        className="card settings-card section"
        onSubmit={(event) => {
          event.preventDefault();
          update.mutate({
            ...form,
            concurrency: clamp(form.concurrency, 1, 16),
            request_timeout_ms: clamp(form.request_timeout_ms, 3000, 120_000),
            refresh_interval_sec: clamp(form.refresh_interval_sec, 30, 3600),
          });
        }}
      >
        <h2 className="section-title">请求</h2>

        {saved && (
          <div className="banner banner--success" role="status">
            <CheckIcon />
            <span>设置已保存。</span>
          </div>
        )}
        {update.isError && (
          <div className="banner banner--danger" role="alert">
            {errorMessage(update.error)}
          </div>
        )}

        <div className="form-row">
          <div className="field">
            <label className="field-label" htmlFor="setting-concurrency">
              并发请求数
            </label>
            <input
              id="setting-concurrency"
              className="input"
              type="number"
              min={1}
              max={16}
              value={form.concurrency}
              onChange={(event) =>
                patch("concurrency", Number(event.target.value))
              }
            />
            <p className="field-hint">同时向服务器发起的请求上限，1–16。</p>
          </div>

          <div className="field">
            <label className="field-label" htmlFor="setting-timeout">
              请求超时（毫秒）
            </label>
            <input
              id="setting-timeout"
              className="input"
              type="number"
              min={3000}
              max={120000}
              step={1000}
              value={form.request_timeout_ms}
              onChange={(event) =>
                patch("request_timeout_ms", Number(event.target.value))
              }
            />
            <p className="field-hint">超时后按网络错误处理，可重试。</p>
          </div>

          <div className="field">
            <label className="field-label" htmlFor="setting-refresh">
              列表刷新间隔（秒）
            </label>
            <input
              id="setting-refresh"
              className="input"
              type="number"
              min={30}
              max={3600}
              step={30}
              value={form.refresh_interval_sec}
              onChange={(event) =>
                patch("refresh_interval_sec", Number(event.target.value))
              }
            />
            <p className="field-hint">只影响作业列表的自动刷新。</p>
          </div>
        </div>

        <div className="section">
          <h3 className="section-title">朗读提交</h3>

          <label className="checkbox-row" htmlFor="setting-dry-run">
            <input
              id="setting-dry-run"
              type="checkbox"
              checked={form.read_aloud_dry_run}
              onChange={(event) =>
                patch("read_aloud_dry_run", event.target.checked)
              }
            />
            <span>
              <span className="checkbox-row-label">只做上传校验与演练</span>
              <span className="checkbox-row-hint">
                协议尚未确认正式提交的请求形状时保持开启；关闭后会调用成绩接口。
              </span>
            </span>
          </label>

          <label className="checkbox-row" htmlFor="setting-keep-audio">
            <input
              id="setting-keep-audio"
              type="checkbox"
              checked={form.keep_audio_files}
              onChange={(event) =>
                patch("keep_audio_files", event.target.checked)
              }
            />
            <span>
              <span className="checkbox-row-label">保留本地录音文件</span>
              <span className="checkbox-row-hint">
                关闭后上传成功即删除本地临时文件。
              </span>
            </span>
          </label>
        </div>

        <div className="form-actions" style={{ marginTop: 16 }}>
          <button
            type="submit"
            className="button button--primary"
            disabled={update.isPending}
          >
            {update.isPending ? "保存中…" : "保存设置"}
          </button>
        </div>
      </form>

      <section
        className="card settings-card section danger-zone"
        aria-labelledby="cache-heading"
      >
        <h2 className="section-title" id="cache-heading">
          本地缓存
        </h2>
        <p className="field-hint">
          清理缓存的作业、书籍和答案数据，不会退出账号，也不会删除录音文件。
        </p>
        {cacheMessage && (
          <div className="banner banner--success" role="status">
            {cacheMessage}
          </div>
        )}
        {clearCache.isError && (
          <div className="banner banner--danger" role="alert">
            {errorMessage(clearCache.error)}
          </div>
        )}
        <button
          type="button"
          className="button button--danger"
          onClick={() => {
            setCacheMessage(null);
            setCacheOpen(true);
          }}
        >
          <TrashIcon />
          清理缓存
        </button>
      </section>

      <ConfirmDialog
        open={cacheOpen}
        title="确认清理本地缓存"
        summary={
          <p>
            将删除本地缓存的作业、书籍与答案数据。账号会话和录音文件不受影响。
          </p>
        }
        confirmLabel="确认清理"
        tone="danger"
        busy={clearCache.isPending}
        onConfirm={() => clearCache.mutate()}
        onCancel={() => setCacheOpen(false)}
      />
    </div>
  );
}

function EmptySettings() {
  return (
    <div className="state-block">
      <p className="state-title">设置数据为空</p>
      <p className="state-text">请重新加载设置。</p>
    </div>
  );
}
