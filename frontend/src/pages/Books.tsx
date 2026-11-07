import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { useAccount } from "../hooks/account";
import { percent } from "../utils";

export function Books() {
  const account = useAccount();
  const books = useQuery({
    queryKey: ["books", account.account_id],
    queryFn: () => api.listBooks(account.account_id),
  });

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">书籍</h1>
          <p className="page-subtitle">
            已购买的教材与同步练习，进入书籍可查看章节和目录作业。
          </p>
        </div>
      </header>

      {books.isPending && <LoadingState label="正在加载书籍…" />}

      {books.isError && (
        <ErrorState error={books.error} onRetry={() => books.refetch()} />
      )}

      {books.isSuccess && books.data.length === 0 && (
        <EmptyState
          title="暂无书籍"
          description="当前账号还没有已购买的书籍。"
        />
      )}

      {books.isSuccess && books.data.length > 0 && (
        <div className="book-grid">
          {books.data.map((book) => (
            <Link
              key={book.book_id}
              to={`/books/${book.book_id}`}
              className="book-card"
            >
              <span className="book-cover" aria-hidden="true">
                {book.name.slice(0, 1)}
              </span>
              <span>
                <span className="book-name">{book.name}</span>
                <span className="book-meta">
                  {book.subject} · 已完成 {book.progress}/{book.total}
                </span>
                <span className="progress-track" aria-hidden="true">
                  <span
                    className="progress-fill"
                    style={{ width: `${percent(book.progress, book.total)}%` }}
                  />
                </span>
              </span>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
