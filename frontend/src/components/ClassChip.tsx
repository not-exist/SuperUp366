export function ClassChip({ label }: { label?: string }) {
  if (!label) return <span className="chip chip--muted">无班级</span>;
  return <span className="chip">{label}</span>;
}
