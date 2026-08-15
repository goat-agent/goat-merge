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
            const head = flying.at(0);
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
                      : "Nothing is being verified right now."
                  }
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

                <QueueBoard
                  label="Waiting"
                  note={head ? `behind #${head.pull_request}` : "nothing ahead"}
                  rows={waiting}
                  chosen={chosen}
                  onChoose={choose}
                  roomy
                  nothing="Nobody is waiting for a turn."
                />

                <QueueBoard
                  label="Merged"
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
