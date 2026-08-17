import { Zap } from "lucide-react";

import type { Row } from "@/shared/api";
import { ago, between, cn } from "@/shared/lib";

import { Standing } from "./Standing";

function waited(row: Row): string {
  return row.settled_at ? between(row.requested_at, row.settled_at) : ago(row.requested_at);
}

export function EntryLine({
  row,
  place,
  chosen,
  onChoose,
  shows,
}: {
  row: Row;
  place?: number;
  chosen: number | null;
  onChoose: (pullRequest: number) => void;
  shows: "identity" | "everything";
}) {
  const picked = chosen === row.pull_request;
  return (
    <li>
      <button
        type="button"
        onClick={() => onChoose(row.pull_request)}
        aria-current={picked}
        className={cn(
          "flex w-full items-baseline gap-3 border-b border-hairline px-3 py-1.5 text-left text-ui transition-colors last:border-b-0",
          picked ? "bg-fill-active" : "hover:bg-fill-hover",
        )}
      >
        <span className="w-place shrink-0 text-right font-mono text-mono text-ink-faint">
          {place ?? ""}
        </span>
        <span className="w-number shrink-0 font-mono text-mono text-ink-faint">
          #{row.pull_request}
        </span>
        <span className="min-w-0 flex-1 truncate text-ink">{row.title || "untitled"}</span>
        {row.expedited_by ? <Zap className="size-3.5 shrink-0 self-center text-warning" /> : null}
        {shows === "everything" ? (
          <>
            <span className="w-standing shrink-0">
              <Standing status={row.status} />
            </span>
            <span className="w-detail-column shrink-0 truncate text-ink-faint" title={row.detail}>
              {row.detail}
            </span>
          </>
        ) : null}
        <span className="shrink-0 text-ink-faint">{row.requested_by}</span>
        <span className="w-waited shrink-0 text-right tabular-nums text-ink-faint">
          {waited(row)}
        </span>
      </button>
    </li>
  );
}
