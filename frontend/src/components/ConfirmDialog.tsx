import { useEffect, useRef, type ReactNode } from "react";

/**
 * Native modal confirmation. Used for every write: the caller passes a
 * human-readable summary of exactly what will be submitted.
 */
export function ConfirmDialog({
  open,
  title,
  summary,
  confirmLabel = "确认提交",
  cancelLabel = "取消",
  tone = "default",
  busy = false,
  hideCancel = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  summary: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: "default" | "danger";
  busy?: boolean;
  hideCancel?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  if (!open) return null;

  return (
    <dialog
      ref={ref}
      data-testid="confirm-dialog"
      className="dialog"
      aria-labelledby="confirm-dialog-title"
      aria-describedby="confirm-dialog-summary"
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) onCancel();
      }}
    >
      <h2 id="confirm-dialog-title" className="dialog-title">
        {title}
      </h2>
      <div id="confirm-dialog-summary" className="dialog-text">
        {summary}
      </div>
      <div className="dialog-actions">
        {!hideCancel && (
          <button
            type="button"
            className="button"
            data-testid="confirm-cancel"
            onClick={onCancel}
            disabled={busy}
          >
            {cancelLabel}
          </button>
        )}
        <button
          type="button"
          data-testid="confirm-submit"
          className={
            tone === "danger" ? "button button--danger" : "button button--primary"
          }
          onClick={onConfirm}
          disabled={busy}
        >
          {busy ? "处理中…" : confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
