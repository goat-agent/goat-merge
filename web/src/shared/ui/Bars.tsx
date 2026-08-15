import { cn } from "@/shared/lib";

export type Point = { label: string; value: number };

export function Columns({ points, tone = "bg-success" }: { points: Point[]; tone?: string }) {
  const most = Math.max(1, ...points.map((point) => point.value));
  const first = points.at(0);
  const last = points.at(-1);
  return (
    <div className="space-y-1.5">
      <div className="flex h-24 items-end gap-1 border-b border-hairline">
        {points.map((point) => (
          <span
            key={point.label}
            title={`${point.label}: ${point.value}`}
            className={cn("min-w-0 flex-1 rounded-t-sm", point.value === 0 ? null : tone)}
            style={{ height: `${(point.value / most) * 100}%` }}
          />
        ))}
      </div>
      <div className="flex justify-between text-caption uppercase text-ink-faint">
        <span>{first?.label}</span>
        <span>{last?.label}</span>
      </div>
    </div>
  );
}

export function Ranked({ points, tone = "bg-danger" }: { points: Point[]; tone?: string }) {
  const sorted = [...points].sort((one, other) => other.value - one.value);
  const most = Math.max(1, ...sorted.map((point) => point.value));
  return (
    <ul className="space-y-2">
      {sorted.map((point) => (
        <li key={point.label} className="flex items-center gap-3 text-ui">
          <span className="min-w-0 flex-1 truncate text-ink-soft">{point.label}</span>
          <span className="w-1/3 shrink-0">
            <span
              className={cn("block h-2 rounded-sm", tone)}
              style={{ width: `${(point.value / most) * 100}%` }}
            />
          </span>
          <span className="w-8 shrink-0 text-right tabular-nums text-ink-faint">{point.value}</span>
        </li>
      ))}
    </ul>
  );
}
