import { Link, useParams, useSearchParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/commands";
import { EmptyState } from "../components/EmptyState";
import { ErrorState } from "../components/ErrorState";
import { LoadingState } from "../components/LoadingState";
import { ArrowLeftIcon } from "../components/Icons";
import { useAccount } from "../hooks/account";

/**
 * Read-only view of one book assignment.
 *
 * A book task id is not a job id, so this does not reuse the job pages: the
 * backend asks the official reader endpoints for this task's words or question
 * rows. The three cases are a vocabulary task, a paper with recordings, and an
 * ordinary paper.
 */
/** The read-aloud route for one class-book page. */
function readAloudPath(
  bookId: string,
  bookType: number,
  courseId: string,
  taskId: string,
  name: string,
  back: string,
): string {
  const params = new URLSearchParams({
    type: String(bookType),
    course: courseId,
    name,
    back,
  });
  return `/books/${bookId}/tasks/${taskId}/readaloud?${params.toString()}`;
}

export function BookTask() {
  const { bookId, taskId } = useParams();
  const [params] = useSearchParams();
  const account = useAccount();
  const bookType = Number(params.get("type")) || 1;
  const courseId = params.get("course");
  const kind = params.get("kind");
  const name = params.get("name") || "书本作业";
  const wordTask = kind === "word_pk";

  const task = useQuery({
    queryKey: [
      "book-task",
      account.account_id,
      bookId,
      courseId,
      taskId,
      wordTask,
    ],
    queryFn: () =>
      api.getBookTask(
        account.account_id,
        bookId ?? "",
        courseId,
        taskId ?? "",
        wordTask,
      ),
    enabled: Boolean(bookId && taskId),
  });

  const back = `/books/${bookId}?type=${bookType}${courseId ? `&course=${courseId}` : ""}`;
  const subtitle = task.data
    ? `${task.data.kind === "word_pk" ? "单词任务" : task.data.read_aloud ? "含跟读" : "作业"} · 只读查看`
    : "逐题查看书本作业。";

  return (
    <div className="page">
      <header className="page-header">
        <div>
          <h1 className="page-title">{name}</h1>
          <p className="page-subtitle">{subtitle}</p>
        </div>
        <div className="page-actions">
          <Link to={back} className="button">
            <ArrowLeftIcon />
            返回书籍
          </Link>
        </div>
      </header>

      {!taskId && (
        <EmptyState title="缺少任务编号" description="请从书籍详情重新进入。" />
      )}

      {task.isPending && Boolean(taskId) && (
        <LoadingState label="正在加载任务内容…" />
      )}

      {task.isError && (
        <ErrorState error={task.error} onRetry={() => task.refetch()} />
      )}

      {task.isSuccess && task.data.read_aloud && bookType === 2 && (
        <div className="banner banner--info" role="note">
          <strong>可提交</strong>
          <span>班级课本的朗读任务可以直接提交，不必先出现在作业列表中。</span>
          <Link
            className="button button--primary"
            to={readAloudPath(
              bookId ?? "",
              bookType,
              courseId ?? "",
              taskId ?? "",
              name,
              back,
            )}
          >
            去提交朗读
          </Link>
        </div>
      )}

      {task.isSuccess && task.data.read_aloud && bookType !== 2 && (
        <div className="banner banner--info" role="note">
          <strong>只读</strong>
          <span>
            该书本任务不在作业列表中，找不到提交所需的作业编号；这里只展示已提交的录音。
          </span>
        </div>
      )}

      {task.isSuccess && task.data.words.length > 0 && (
        <div className="word-grid">
          {task.data.words.map((word) => (
            <div
              key={word.word}
              className={`word-card${word.mastered ? " word-card--known" : ""}`}
              data-testid="book-word"
            >
              <div className="word-head">
                <span className="word-title">{word.word}</span>
                <span
                  className={`badge ${word.mastered ? "badge--finished" : "badge--unknown"}`}
                >
                  {word.mastered ? "已掌握" : "未掌握"}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      {task.isSuccess &&
        task.data.words.length === 0 &&
        task.data.questions.length === 0 && (
          <EmptyState
            title="暂无内容"
            description="该任务还没有可查看的题目。"
          />
        )}

      {task.isSuccess && task.data.questions.length > 0 && (
        <div className="stack">
          {task.data.questions.map((question, index) => (
            <article
              key={question.question_id || index}
              className="question-card"
              data-testid="book-question"
            >
              <div className="question-head">
                <span className="question-no">
                  第 {question.number || index + 1} 题
                </span>
                <span className="job-sub">
                  {[question.type_name, question.knowledge]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </div>

              <div className="answer-grid">
                {question.read_aloud ? (
                  <div>
                    <div className="stat-label">我的录音</div>
                    <div className="answer-value">
                      {question.my_answer ? (
                        <audio
                          controls
                          preload="none"
                          src={question.my_answer}
                          style={{ width: "100%" }}
                        />
                      ) : (
                        "暂无录音"
                      )}
                    </div>
                  </div>
                ) : (
                  <>
                    <div>
                      <div className="stat-label">我的答案</div>
                      <div className="answer-value">
                        {question.my_answer || "（空）"}
                      </div>
                    </div>
                    <div>
                      <div className="stat-label">正确答案</div>
                      <div className="answer-value">
                        {question.correct_answer || "—"}
                      </div>
                    </div>
                  </>
                )}
                <div>
                  <div className="stat-label">得分</div>
                  <div className="answer-value">
                    {question.score ?? "—"} / {question.max_score ?? "—"}
                  </div>
                </div>
                {question.class_average != null && (
                  <div>
                    <div className="stat-label">班级平均分</div>
                    <div className="answer-value">{question.class_average}</div>
                  </div>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
