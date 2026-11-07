import type { JobStatusKind } from "../api/types";

export const STATUS_LABELS: Record<JobStatusKind, string> = {
  finished: "已完成",
  returned: "已退回",
  expired: "已过期",
  pending: "待完成",
  unknown: "未知",
};

export const STATUS_OPTIONS: JobStatusKind[] = [
  "pending",
  "returned",
  "finished",
  "expired",
  "unknown",
];

export function StatusBadge({ kind }: { kind: JobStatusKind }) {
  return (
    <span className={`badge badge--${kind}`}>
      <span className="badge__dot" aria-hidden="true" />
      {STATUS_LABELS[kind]}
    </span>
  );
}
