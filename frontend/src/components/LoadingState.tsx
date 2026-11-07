export function LoadingState({ label = "正在加载…" }: { label?: string }) {
  return (
    <div className="state-block" role="status" aria-live="polite">
      <span className="state-icon">
        <svg
          width="22"
          height="22"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.8"
          strokeLinecap="round"
          aria-hidden="true"
          focusable="false"
        >
          <path d="M12 3a9 9 0 1 0 9 9" />
        </svg>
      </span>
      <p className="state-text">{label}</p>
    </div>
  );
}
