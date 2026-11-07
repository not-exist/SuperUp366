import type { ReactNode } from "react";
import { InboxIcon } from "./Icons";

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="state-block">
      <span className="state-icon">
        <InboxIcon width={22} height={22} />
      </span>
      <p className="state-title">{title}</p>
      {description && <p className="state-text">{description}</p>}
      {action}
    </div>
  );
}
