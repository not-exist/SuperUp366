export type ProgressState = "pending" | "running" | "done" | "failed";

export interface ProgressEntry {
  id: string;
  label: string;
  status: ProgressState;
  detail?: string;
}

const LABELS: Record<ProgressState, string> = {
  pending: "待处理",
  running: "处理中",
  done: "已完成",
  failed: "失败",
};

export function ProgressList({ items }: { items: ProgressEntry[] }) {
  return (
    <ul className="progress-list" aria-label="处理进度" aria-live="polite">
      {items.map((item) => (
        <li
          key={item.id}
          className={`progress-item progress-item--${item.status}`}
        >
          <span className="progress-dot" aria-hidden="true" />
          <span className="progress-label" title={item.label}>
            {item.label}
          </span>
          <span className="progress-status">
            {item.detail ?? LABELS[item.status]}
          </span>
        </li>
      ))}
    </ul>
  );
}
