import { useParams, useSearchParams } from "react-router-dom";

import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { useAsked, useEvery, useLive } from "@/shared/lib";
import { DetailPanel } from "@/widgets/detail-panel";
import { QueueBoard } from "@/widgets/queue-board";
import { RepositoryHeader } from "@/widgets/repository-header";

export function HistoryPage() {
  const { owner = "", name = "", branch = "" } = useParams();
  const beat = useLive() + useEvery(30);
  const [asking, setAsking] = useSearchParams();
  const chosen = Number(asking.get("pull")) || null;
  const asked = useAsked(() => api.history(owner, name, branch), [owner, name, branch], beat);

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col">
        <RepositoryHeader
          owner={owner}
          name={name}
          branch={branch}
          section="history"
          right={<span className="text-ui text-ink-faint">what already happened</span>}
        />

        <WhatWeHave asked={asked} of="what already happened">
          {(rows) => (
            <div className="min-h-0 flex-1 overflow-y-auto">
              <QueueBoard
                rows={rows}
                chosen={chosen}
                onChoose={(pull) => setAsking(pull === null ? {} : { pull: String(pull) })}
                numbered={false}
              />
            </div>
          )}
        </WhatWeHave>
      </div>
      <DetailPanel owner={owner} name={name} number={chosen} beat={beat} onChanged={asked.again} />
    </div>
  );
}
