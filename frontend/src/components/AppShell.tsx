import { NavLink, Outlet, useNavigate } from "react-router-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/commands";
import type { Account } from "../api/types";
import { AccountContext } from "../hooks/account";
import { errorMessage, initialOf } from "../utils";
import {
  BookIcon,
  JobsIcon,
  LogoutIcon,
  SettingsIcon,
  WrongNoteIcon,
} from "./Icons";

const NAV = [
  { to: "/jobs", label: "作业", Icon: JobsIcon },
  { to: "/books", label: "书籍", Icon: BookIcon },
  { to: "/wrong-notes", label: "错题本", Icon: WrongNoteIcon },
  { to: "/settings", label: "设置", Icon: SettingsIcon },
];

export function AppShell({ account }: { account: Account }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const logout = useMutation({
    mutationFn: () => api.logout(account.account_id),
    onSuccess: async () => {
      await queryClient.invalidateQueries();
      navigate("/login", { replace: true });
    },
  });

  return (
    <AccountContext.Provider value={account}>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand">
            <div className="brand-mark" aria-hidden="true">
              U
            </div>
            <div>
              <div className="brand-text">SuperUp366</div>
              <div className="brand-sub">学生端</div>
            </div>
          </div>

          <nav className="nav" aria-label="主导航">
            {NAV.map(({ to, label, Icon }) => (
              <NavLink
                key={to}
                to={to}
                className={({ isActive }) =>
                  `nav-link${isActive ? " nav-link--active" : ""}`
                }
              >
                <Icon />
                <span>{label}</span>
              </NavLink>
            ))}
          </nav>

          <div className="sidebar-footer">
            <div className="account-summary">
              <span className="avatar" aria-hidden="true">
                {initialOf(account.display_name)}
              </span>
              <div>
                <div className="account-name">{account.display_name}</div>
                <div className="account-meta">{account.school_name}</div>
              </div>
            </div>
            <div className="account-actions">
              <NavLink
                to="/login"
                className="button button--sm button--ghost"
              >
                切换账号
              </NavLink>
              <button
                type="button"
                className="button button--sm button--ghost icon-button"
                aria-label="退出登录"
                title="退出登录"
                onClick={() => logout.mutate()}
                disabled={logout.isPending}
              >
                <LogoutIcon />
              </button>
            </div>
            {logout.isError && (
              <p className="field-hint" role="alert">
                {errorMessage(logout.error)}
              </p>
            )}
          </div>
        </aside>

        <main className="main">
          <Outlet />
        </main>
      </div>
    </AccountContext.Provider>
  );
}
