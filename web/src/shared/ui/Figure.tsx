export function Figure({ label, value, note }: { label: string; value: string; note?: string }) {
  return (
    <div className="space-y-1 rounded-lg bg-sunken p-4">
      <p className="text-caption uppercase text-ink-faint">{label}</p>
      <p className="text-title tabular-nums text-ink">{value}</p>
      {note ? <p className="text-ui text-ink-faint">{note}</p> : null}
    </div>
  );
}
