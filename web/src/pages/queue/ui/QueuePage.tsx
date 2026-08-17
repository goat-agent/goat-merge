import { useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";

import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import { QueueControls } from "@/features/queue-controls";
import type { Row, Status, Trouble } from "@/shared/api";
import { api } from "@/shared/api";
import { useAsked, useEvery, useLive } from "@/shared/lib";
import { Badge } from "@/shared/ui";
import { DetailPanel } from "@/widgets/detail-panel";
import { QueueBoard } from "@/widgets/queue-board";

const beingVerified: Status[] = ["Preparing", "Checking", "Merging"];

function inFlight(row: Row): boolean {
  return row.settled_at === null && beingVerified.includes(row.status);
}

function stillWaiting(row: Row): boolean {
  return row.settled_at === null && !beingVerified.includes(row.status);
}

function together(atOnce: number): string {
  if (atOnce <= 1) return "one at a time";
  return `up to ${atOnce} together`;
}

function behind(rows: Row[]): string | null {
  const numbers = rows.map((row) => `#${row.pull_request}`);
  if (numbers.length === 0) return null;
  if (numbers.length === 1) return `behind ${numbers[0]}`;
  return `behind ${numbers.slice(0, -1).join(", ")} and ${numbers.at(-1)}`;
}

function howItEnded(rows: Row[]): string {
  const merged = rows.filter((row) => row.status === "Merged").length;
  const turnedAway = rows.length - merged;
  if (turnedAway === 0) return `${merged} merged`;
  return `${merged} merged, ${turnedAway} did not`;
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
            const flying = shown.entries.filter(inFlight);
            const waiting = shown.entries.filter(stillWaiting);
            const note = behind(flying);
            const recent = (landed.answer ?? []).slice(0, 5);
            return (
              <div className="min-h-0 flex-1 overflow-y-auto">
                <QueueBoard
                  label="Being verified"
                  rows={flying}
                  chosen={chosen}
                  onChoose={choose}
                  roomy
                  nothing={
                    waiting.length === 0
                      ? "Nothing is in this queue."
                      : "Nothing is being verified right now, so the queue is not moving."
                  }
                  note={together(shown.verify_at_once)}
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
                />

                {waiting.length > 0 || flying.length > 0 ? (
                  <QueueBoard
                    label="Waiting"
                    {...(note ? { note } : {})}
                    rows={waiting}
                    chosen={chosen}
                    onChoose={choose}
                    roomy
                    nothing="Nobody is waiting for a turn."
                  />
                ) : null}

                <QueueBoard
                  label="Done"
                  note={howItEnded(landed.answer ?? [])}
                  rows={recent}
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
                />
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
