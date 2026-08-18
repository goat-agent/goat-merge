import { useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";

import { EntryLine } from "@/entities/queue-entry";
import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import { QueueControls } from "@/features/queue-controls";
import type { QueueView, Row, Status, Trial, Trouble } from "@/shared/api";
import { api } from "@/shared/api";
import { useAsked, useEvery, useLive } from "@/shared/lib";
import { Badge } from "@/shared/ui";
import { BatchGroup } from "@/widgets/batch-group";
import { DetailPanel } from "@/widgets/detail-panel";
import { QueueBoard } from "@/widgets/queue-board";

const beingVerified: Status[] = ["Preparing", "Checking", "Merging"];

function inFlight(row: Row): boolean {
  return row.settled_at === null && beingVerified.includes(row.status);
}

function inLine(row: Row): boolean {
  return row.settled_at === null && row.queued_at !== null && !inFlight(row);
}

function held(row: Row): boolean {
  return row.settled_at === null && row.queued_at === null;
}

type Riding = { attempt: Trial; rows: Row[] };

function intoGroups(rows: Row[]): { groups: Riding[]; alone: Row[] } {
  const riding = new Map<number, Riding>();
  const alone: Row[] = [];
  for (const row of rows) {
    if (!row.attempt) {
      alone.push(row);
      continue;
    }
    const already = riding.get(row.attempt.id);
    if (already) {
      already.rows.push(row);
    } else {
      riding.set(row.attempt.id, { attempt: row.attempt, rows: [row] });
    }
  }
  const groups = [...riding.values()].sort(
    (one, other) =>
      one.attempt.depth - other.attempt.depth ||
      one.attempt.started_at.localeCompare(other.attempt.started_at),
  );
  return { groups, alone };
}

type Landed = { riding: Riding } | { row: Row };

function inTheOrderTheyLanded(rows: Row[]): Landed[] {
  const seen = new Set<number>();
  const landed: Landed[] = [];
  for (const row of rows) {
    const attempt = row.attempt;
    if (!attempt) {
      landed.push({ row });
      continue;
    }
    if (seen.has(attempt.id)) continue;
    seen.add(attempt.id);
    landed.push({
      riding: { attempt, rows: rows.filter((one) => one.attempt?.id === attempt.id) },
    });
  }
  return landed;
}

function perCiRun(shown: QueueView): string {
  const many = shown.verify_at_once;
  const said = [many <= 1 ? "one at a time" : `${many} per CI run`];
  if (shown.speculate_to > 1) said.push(`${shown.speculate_to} candidates at once`);
  if (shown.verify_at_once_because) said.push(shown.verify_at_once_because);
  return said.join(" · ");
}

export function QueuePage() {
  const { owner = "", name = "", branch = "" } = useParams();
  const beat = useLive() + useEvery(15);
  const [asking, setAsking] = useSearchParams();
  const [wrong, setWrong] = useState<Trouble | null>(null);
  const chosen = Number(asking.get("pull")) || null;
  const asked = useAsked(() => api.queue(owner, name, branch), [owner, name, branch], beat);
  const landed = useAsked(
    () => api.history(owner, name, branch),
    [owner, name, branch],
    useEvery(60),
  );
  const queue = asked.answer;
  const choose = (pull: number) => setAsking({ pull: String(pull) });

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        {wrong ? <WhatWentWrong trouble={wrong} place="banner" /> : null}
        <WhatWeHave asked={asked} of="the queue">
          {(shown) => {
            const flying = intoGroups(shown.entries.filter(inFlight));
            const queued = shown.entries.filter(inLine);
            const waiting = shown.entries.filter(held);
            const recent = inTheOrderTheyLanded((landed.answer ?? []).slice(0, 8));
            return (
              <div className="min-h-0 flex-1 overflow-y-auto pb-6">
                <QueueBoard
                  label="Batched"
                  note={perCiRun(shown)}
                  rows={flying.alone}
                  chosen={chosen}
                  onChoose={choose}
                  shows="everything"
                  nothing="Nothing is being verified. Add the merge-queue label to a pull request to put it in."
                  aside={
                    <div className="flex items-center gap-3">
                      {shown.paused ? (
                        <Badge tone="warning">paused by {shown.paused_by ?? "someone"}</Badge>
                      ) : (
                        <Badge tone="success">active</Badge>
                      )}
                      <QueueControls
                        owner={owner}
                        name={name}
                        branch={branch}
                        paused={shown.paused}
                        onChanged={asked.again}
                        onTrouble={setWrong}
                      />
                    </div>
                  }
                >
                  {flying.groups.map((riding) => (
                    <BatchGroup
                      key={riding.attempt.id}
                      attempt={riding.attempt}
                      rows={riding.rows}
                      owner={owner}
                      name={name}
                    >
                      {riding.rows.map((row, at) => (
                        <EntryLine
                          key={row.pull_request}
                          row={row}
                          place={at + 1}
                          chosen={chosen}
                          onChoose={choose}
                          shows="identity"
                        />
                      ))}
                    </BatchGroup>
                  ))}
                </QueueBoard>

                {queued.length > 0 ? (
                  <QueueBoard
                    label="In line"
                    note="not yet in a batch"
                    rows={queued}
                    chosen={chosen}
                    onChoose={choose}
                    numbered
                    nothing=""
                  />
                ) : null}

                {waiting.length > 0 ? (
                  <QueueBoard
                    label="Held"
                    note="not in line"
                    rows={waiting}
                    chosen={chosen}
                    onChoose={choose}
                    nothing=""
                  />
                ) : null}

                <QueueBoard
                  label="Done"
                  rows={[]}
                  chosen={chosen}
                  onChoose={choose}
                  nothing="Nothing has been through this queue yet."
                  aside={
                    <Link
                      to={`/history/${owner}/${name}/${branch}`}
                      className="text-ui text-primary underline-offset-2 hover:underline"
                    >
                      See all
                    </Link>
                  }
                >
                  {recent.map((one) =>
                    "row" in one ? (
                      <ul key={`row-${one.row.pull_request}`} className="border-b border-hairline">
                        <EntryLine
                          row={one.row}
                          chosen={chosen}
                          onChoose={choose}
                          shows="everything"
                        />
                      </ul>
                    ) : (
                      <BatchGroup
                        key={one.riding.attempt.id}
                        attempt={one.riding.attempt}
                        rows={one.riding.rows}
                        owner={owner}
                        name={name}
                      >
                        {one.riding.rows.map((row) => (
                          <EntryLine
                            key={row.pull_request}
                            row={row}
                            chosen={chosen}
                            onChoose={choose}
                            shows="identity"
                          />
                        ))}
                      </BatchGroup>
                    ),
                  )}
                </QueueBoard>
              </div>
            );
          }}
        </WhatWeHave>
      </div>

      <DetailPanel
        owner={owner}
        name={name}
        number={chosen}
        queue={queue}
        beat={beat}
        onChanged={asked.again}
      />
    </div>
  );
}
