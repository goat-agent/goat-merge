import { Zap } from "lucide-react";
import type { ReactNode } from "react";

import { Standing } from "@/entities/queue-entry";
import type { Row } from "@/shared/api";
import { ago, between, cn } from "@/shared/lib";

function waited(row: Row): string {
  return row.settled_at ? between(row.requested_at, row.settled_at) : ago(row.requested_at);
}

function Line({
  row,
  chosen,
  onChoose,
  roomy,
}: {
  row: Row;
  chosen: number | null;
  onChoose: (pullRequest: number) => void;
  roomy: boolean;
}) {
  const picked = chosen === row.pull_request;
  return (
    <li>
      <button
        type="button"
        onClick={() => onChoose(row.pull_request)}
        aria-current={picked}
        className={cn(
          "flex w-full flex-col gap-1 border-b border-hairline px-4 py-2.5 text-left text-ui transition-colors",
          picked ? "bg-fill-active" : "hover:bg-fill-hover",
        )}
      >
        <span className="flex w-full items-baseline gap-3">
          <span className="shrink-0 font-mono text-mono text-ink-faint">#{row.pull_request}</span>
          <span className="min-w-0 flex-1 truncate text-ink">{row.title || "untitled"}</span>
          {row.expedited_by ? <Zap className="size-3.5 shrink-0 self-center text-warning" /> : null}
          {roomy ? null : <Standing status={row.status} />}
          <span className="shrink-0 tabular-nums text-ink-faint">{waited(row)}</span>
        </span>
        {roomy ? (
          <span className="flex w-full items-baseline gap-3">
            <Standing status={row.status} />
            <span className="min-w-0 flex-1 truncate text-ink-faint">{row.detail}</span>
            <span className="shrink-0 text-ink-faint">asked by {row.requested_by}</span>
          </span>
        ) : null}
      </button>
    </li>
  );
}

export function QueueBoard({
  label,
  note,
  rows,
  chosen,
  onChoose,
  roomy = false,
  nothing,
  aside,
}: {
  label: string;
  note?: string;
  rows: Row[];
  chosen: number | null;
  onChoose: (pullRequest: number) => void;
  roomy?: boolean;
  nothing: string;
  aside?: ReactNode;
}) {
  return (
    <section>
      <header className="flex h-header items-center gap-3 px-4">
        <h2 className="text-caption uppercase text-ink-faint">{label}</h2>
        {note ? <span className="text-ui text-ink-faint">{note}</span> : null}
        <span className="flex-1" />
        {aside}
      </header>
      {rows.length === 0 ? (
        <p className="border-b border-hairline px-4 pb-4 text-ui text-ink-faint">{nothing}</p>
      ) : (
        <ul>
          {rows.map((row) => (
            <Line
              key={row.pull_request}
              row={row}
              chosen={chosen}
              onChoose={onChoose}
              roomy={roomy}
            />
          ))}
        </ul>
      )}
    </section>
  );
}
