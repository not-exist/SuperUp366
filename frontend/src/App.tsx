import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/AppShell";
import { ErrorState } from "./components/ErrorState";
import { useAccounts } from "./hooks/account";
import { AnswerView } from "./pages/AnswerView";
import { AppliedWriting } from "./pages/AppliedWriting";
import { BookDetail } from "./pages/BookDetail";
import { BookTask } from "./pages/BookTask";
import { Books } from "./pages/Books";
import { Jobs } from "./pages/Jobs";
import { Login } from "./pages/Login";
import { ReadAloud } from "./pages/ReadAloud";
import { SettingsPage } from "./pages/Settings";
import { WordPk } from "./pages/WordPk";
import { WrongNote } from "./pages/WrongNote";

export default function App() {
  const accounts = useAccounts();

  if (accounts.isLoading) {
    return (
      <div className="boot-screen" role="status">
        正在读取账号…
      </div>
    );
  }

  if (accounts.isError) {
    return (
      <div className="boot-screen">
        <ErrorState
          error={accounts.error}
          onRetry={() => accounts.refetch()}
          title="无法读取账号"
        />
      </div>
    );
  }

  const active = accounts.data?.find((account) => account.active) ?? null;

  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      {active ? (
        <Route element={<AppShell account={active} />}>
          <Route index element={<Navigate to="/jobs" replace />} />
          <Route path="jobs" element={<Jobs />} />
          <Route path="jobs/:jobId/answer" element={<AnswerView />} />
          <Route path="jobs/:jobId/writing" element={<AppliedWriting />} />
          <Route path="jobs/:jobId/wordpk" element={<WordPk />} />
          <Route path="jobs/:jobId/readaloud" element={<ReadAloud />} />
          <Route path="books" element={<Books />} />
          <Route path="books/:bookId" element={<BookDetail />} />
          <Route path="books/:bookId/tasks/:taskId" element={<BookTask />} />
          <Route
            path="books/:bookId/tasks/:taskId/readaloud"
            element={<ReadAloud />}
          />
          <Route path="wrong-notes" element={<WrongNote />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/jobs" replace />} />
        </Route>
      ) : (
        <Route path="*" element={<Navigate to="/login" replace />} />
      )}
    </Routes>
  );
}
