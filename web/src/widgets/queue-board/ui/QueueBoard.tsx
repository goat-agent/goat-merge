import { Children, type ReactNode } from "react";

import { EntryLine } from "@/entities/queue-entry";
import type { Row } from "@/shared/api";

function saidByEveryone(rows: Row[]): string | null {
  if (rows.length < 2) return null;
  const first = rows[0]?.detail ?? "";
  if (!first) return null;
  return rows.every((row) => row.detail === first) ? first : null;
}

export function QueueBoard({
  label,
  note,
  rows,
  chosen,
  onChoose,
  numbered = false,
  shows = "everything",
  nothing,
  aside,
  children,
}: {
  label: string;
  note?: string;
  rows: Row[];
  chosen: number | null;
  onChoose: (pullRequest: number) => void;
  numbered?: boolean;
  shows?: "identity" | "everything";
  nothing: string;
  aside?: ReactNode;
  children?: ReactNode;
}) {
  const everyone = saidByEveryone(rows);
  return (
    <section>
      <header className="flex h-header items-center gap-3 px-4">
        <h2 className="text-caption uppercase text-ink-faint">{label}</h2>
        {note ? <span className="text-ui text-ink-faint">{note}</span> : null}
        {everyone ? <span className="text-ui text-ink-faint">{everyone}</span> : null}
        <span className="flex-1" />
        {aside}
      </header>
      {children}
      {rows.length === 0 && Children.count(children) === 0 ? (
        <p className="border-b border-hairline px-4 pb-4 text-ui text-ink-faint">{nothing}</p>
      ) : null}
      {rows.length > 0 ? (
        <ul className="border-b border-hairline">
          {rows.map((row, at) => (
            <EntryLine
              key={row.pull_request}
              row={row}
              {...(numbered ? { place: at + 1 } : {})}
              chosen={chosen}
              onChoose={onChoose}
              shows={everyone ? "identity" : shows}
            />
          ))}
        </ul>
      ) : null}
    </section>
  );
}
