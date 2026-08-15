import { Link, useParams, useSearchParams } from "react-router-dom";

import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { spellOut, useAsked, useEvery, useLive } from "@/shared/lib";
import { Columns, Figure, Panel, Ranked } from "@/shared/ui";
import { DetailPanel } from "@/widgets/detail-panel";
import { QueueBoard } from "@/widgets/queue-board";

export function HistoryPage() {
  const { owner = "", name = "", branch = "" } = useParams();
  const beat = useLive() + useEvery(30);
  const [asking, setAsking] = useSearchParams();
  const chosen = Number(asking.get("pull")) || null;
  const asked = useAsked(() => api.history(owner, name, branch), [owner, name, branch], beat);
  const numbers = useAsked(
    () => api.insights(owner, name, branch),
    [owner, name, branch],
    useEvery(60),
  );
  const said = numbers.answer;

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col overflow-y-auto">
        <div className="space-y-6 p-6">
          <header className="flex items-baseline justify-between gap-3">
            <h1 className="text-title text-ink">What already happened</h1>
            <Link
              to={`/queue/${owner}/${name}/${branch}`}
              className="text-ui text-primary underline-offset-2 hover:underline"
            >
              Back to the queue
            </Link>
          </header>

          {said ? (
            <>
              <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
                <Figure label="Merged" value={String(said.merged)} />
                <Figure
                  label="Median to merge"
                  value={spellOut(said.median_wait_seconds)}
                  note="half of them were faster than this"
                />
                <Figure
                  label="95th percentile"
                  value={spellOut(said.p95_wait_seconds)}
                  note="one in twenty was slower than this"
                />
                <Figure
                  label="Left the queue"
                  value={String(said.failed + said.cancelled)}
                  note={`${said.failed} failed, ${said.cancelled} cancelled`}
                />
              </div>

              <Panel label="Merged per day">
                {said.merged_by_day.length === 0 ? (
                  <p className="text-ui text-ink-faint">
                    Nothing has merged through this queue yet.
                  </p>
                ) : (
                  <Columns
                    points={said.merged_by_day.map((day) => ({
                      label: day.day,
                      value: day.count,
                    }))}
                  />
                )}
              </Panel>

              <Panel label="Why pull requests left the queue">
                {said.reasons.length === 0 ? (
                  <p className="text-ui text-ink-faint">Nothing has been turned away.</p>
                ) : (
                  <Ranked
                    points={said.reasons.map((reason) => ({
                      label: reason.reason,
                      value: reason.count,
                    }))}
                  />
                )}
              </Panel>
            </>
          ) : null}
        </div>

        <WhatWeHave asked={asked} of="what already happened">
          {(rows) => (
            <QueueBoard
              label="Everything that has been through"
              rows={rows}
              chosen={chosen}
              onChoose={(pull) => setAsking({ pull: String(pull) })}
              nothing="Nothing has been through this queue yet."
            />
          )}
        </WhatWeHave>
      </div>

      <DetailPanel
        owner={owner}
        name={name}
        number={chosen}
        queue={null}
        beat={beat}
        onChanged={asked.again}
      />
    </div>
  );
}
