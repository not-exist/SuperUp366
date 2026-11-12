import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { Account } from "../api/types";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { PlusIcon } from "../components/Icons";
import { useAccounts } from "../hooks/account";
import { errorMessage, initialOf } from "../utils";

export function Login() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const accounts = useAccounts();

  const [formOpen, setFormOpen] = useState(false);
  const [target, setTarget] = useState<Account | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");

  const showForm = formOpen || (accounts.data?.length ?? 0) === 0;

  const switchAccount = useMutation({
    mutationFn: (accountId: string) => api.switchAccount(accountId),
    onSuccess: async () => {
      // Drop every cached page: another account's counts must never leak into
      // this session.
      await queryClient.invalidateQueries();
      navigate("/jobs", { replace: true });
    },
  });

  const login = useMutation({
    mutationFn: () => api.login(username.trim(), password),
    onSuccess: async () => {
      // A re-login must not keep the counts cached before the session expired.
      await queryClient.invalidateQueries();
      setPassword("");
      setFormOpen(false);
      navigate("/jobs", { replace: true });
    },
  });

  function openRelogin(account: Account) {
    setTarget(account);
    setUsername(account.username);
    setPassword("");
    setFormOpen(true);
    login.reset();
  }

  function openNewAccount() {
    setTarget(null);
    setUsername("");
    setPassword("");
    setFormOpen(true);
    login.reset();
  }

  function cancelForm() {
    setFormOpen(false);
    setTarget(null);
    setPassword("");
    login.reset();
  }

  return (
    <div className="login-page">
      <div className="login-card">
        <header className="login-header">
          <div className="brand-mark" aria-hidden="true">
            U
          </div>
          <div>
            <h1 className="login-title">SuperUp366</h1>
            <p className="login-subtitle">
              选择账号进入，或登录另一个账号。密码只用于本次登录，不会保存在本地。
            </p>
          </div>
        </header>

        {accounts.isLoading && <LoadingState label="正在读取账号…" />}

        {accounts.isError && (
          <ErrorState
            error={accounts.error}
            onRetry={() => accounts.refetch()}
          />
        )}

        {accounts.data && accounts.data.length > 0 && (
          <div className="account-list">
            {accounts.data.map((account) => (
              <div
                key={account.account_id}
                className={`account-row${account.active ? " account-row--active" : ""}`}
              >
                <div className="avatar" aria-hidden="true">
                  {initialOf(account.display_name)}
                </div>
                <div className="account-row-main">
                  <div className="account-name">
                    {account.display_name}
                    {account.active && (
                      <span className="badge badge--finished">当前</span>
                    )}
                    {!account.session_valid && (
                      <span className="badge badge--returned">需要重新登录</span>
                    )}
                  </div>
                  <div className="account-meta">
                    {account.school_name} · {account.username}
                  </div>
                </div>
                <div className="account-actions">
                  {!account.active && account.session_valid && (
                    <button
                      type="button"
                      className="button button--sm"
                      onClick={() => switchAccount.mutate(account.account_id)}
                      disabled={switchAccount.isPending}
                    >
                      切换
                    </button>
                  )}
                  <button
                    type="button"
                    className="button button--sm button--ghost"
                    onClick={() => openRelogin(account)}
                  >
                    重新登录
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {switchAccount.isError && (
          <div className="banner banner--danger" role="alert">
            {errorMessage(switchAccount.error)}
          </div>
        )}

        {accounts.data && accounts.data.length === 0 && !showForm && (
          <EmptyState
            title="还没有保存的账号"
            description="使用天学网账号登录后，会话会保存在系统凭据库中。"
          />
        )}

        {!showForm && (
          <button type="button" className="button" onClick={openNewAccount}>
            <PlusIcon />
            登录其他账号
          </button>
        )}

        {showForm && (
          <form
            className="login-form"
            onSubmit={(event) => {
              event.preventDefault();
              login.mutate();
            }}
          >
            <h2 className="section-title">
              {target ? `重新登录：${target.display_name}` : "登录账号"}
            </h2>

            {login.isError && (
              <div className="banner banner--danger" role="alert">
                {errorMessage(login.error)}
              </div>
            )}

            <div className="field">
              <label className="field-label" htmlFor="login-username">
                账号
              </label>
              <input
                id="login-username"
                className="input"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                autoComplete="username"
                required
              />
            </div>

            <div className="field">
              <label className="field-label" htmlFor="login-password">
                密码
              </label>
              <input
                id="login-password"
                className="input"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                autoComplete="current-password"
                required
              />
              <p className="field-hint">
                登录成功后只保存会话票据，不保存密码。
              </p>
            </div>

            <div className="form-actions">
              {(accounts.data?.length ?? 0) > 0 && (
                <button
                  type="button"
                  className="button button--ghost"
                  onClick={cancelForm}
                >
                  取消
                </button>
              )}
              <button
                type="submit"
                className="button button--primary"
                disabled={login.isPending}
              >
                {login.isPending ? "登录中…" : "登录"}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
