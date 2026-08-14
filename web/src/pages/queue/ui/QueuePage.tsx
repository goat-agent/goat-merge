import { useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";

import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import { QueueControls } from "@/features/queue-controls";
import type { Trouble } from "@/shared/api";
import { api } from "@/shared/api";
import { useAsked, useEvery, useLive } from "@/shared/lib";
import { Badge } from "@/shared/ui";
import { DetailPanel } from "@/widgets/detail-panel";
import { QueueBoard } from "@/widgets/queue-board";
import { RepositoryHeader } from "@/widgets/repository-header";

export function QueuePage() {
  const { owner = "", name = "", branch = "" } = useParams();
  const beat = useLive() + useEvery(15);
  const [asking, setAsking] = useSearchParams();
  const [wrong, setWrong] = useState<Trouble | null>(null);
  const chosen = Number(asking.get("pull")) || null;
  const asked = useAsked(() => api.queue(owner, name, branch), [owner, name, branch], beat);
  const queue = asked.answer;

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        <RepositoryHeader
          owner={owner}
          name={name}
          branch={branch}
          section="queue"
          right={
            queue ? (
              <div className="flex items-center gap-3">
                {queue.paused ? (
                  <Badge tone="warning">paused by {queue.paused_by ?? "someone"}</Badge>
                ) : (
                  <Badge tone="success">active</Badge>
                )}
                <QueueControls
                  owner={owner}
                  name={name}
                  branch={branch}
                  paused={queue.paused}
                  onChanged={asked.again}
                  onTrouble={setWrong}
                />
              </div>
            ) : null
          }
        />

        {wrong ? <WhatWentWrong trouble={wrong} place="banner" /> : null}
        <WhatWeHave asked={asked} of="the queue">
          {(shown) => (
            <div className="min-h-0 flex-1 overflow-y-auto">
              <QueueBoard
                rows={shown.entries}
                chosen={chosen}
                onChoose={(pull) => setAsking(pull === null ? {} : { pull: String(pull) })}
              />
            </div>
          )}
        </WhatWeHave>
      </div>

      <DetailPanel owner={owner} name={name} number={chosen} beat={beat} onChanged={asked.again} />
    </div>
  );
}
