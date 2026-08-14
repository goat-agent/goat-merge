import { Zap } from "lucide-react";

import { Standing } from "@/entities/queue-entry";
import type { Row } from "@/shared/api";
import { ago, between, cn } from "@/shared/lib";
import { Empty } from "@/shared/ui";

type Placed = { row: Row; place: number | null };

function placed(rows: Row[], numbered: boolean): Placed[] {
  let next = 0;
  return rows.map((row) => {
    const running = numbered && row.queued_at !== null && row.settled_at === null;
    if (running) next += 1;
    return { row, place: running ? next : null };
  });
}

export function QueueBoard({
  rows,
  chosen,
  onChoose,
  numbered = true,
}: {
  rows: Row[];
  chosen: number | null;
  onChoose: (pullRequest: number) => void;
  numbered?: boolean;
}) {
  if (rows.length === 0) {
    return <Empty>Nothing is in this queue.</Empty>;
  }

  return (
    <ul>
      {placed(rows, numbered).map(({ row, place }) => (
        <li key={row.pull_request} className="border-b border-hairline last:border-b-0">
          <button
            type="button"
            onClick={() => onChoose(row.pull_request)}
            aria-current={chosen === row.pull_request}
            className={cn(
              "flex w-full items-center gap-3 px-4 py-2 text-left text-ui transition-colors",
              chosen === row.pull_request ? "bg-fill-active" : "hover:bg-fill-hover",
            )}
          >
            {numbered ? (
              <span className="w-place shrink-0 text-right tabular-nums text-ink-faint">
                {place ?? "—"}
              </span>
            ) : null}
            <span className="w-number shrink-0 font-mono text-ink-faint">#{row.pull_request}</span>
            <span className="min-w-0 flex-1 truncate text-ink">{row.title || "untitled"}</span>
            {row.expedited_by ? <Zap className="size-3.5 shrink-0 text-warning" /> : null}
            <span className="w-detail-column shrink-0 truncate text-ink-faint">{row.detail}</span>
            <span className="w-standing shrink-0">
              <Standing status={row.status} />
            </span>
            <span className="w-waited shrink-0 text-right tabular-nums text-ink-faint">
              {row.settled_at ? between(row.requested_at, row.settled_at) : ago(row.requested_at)}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
