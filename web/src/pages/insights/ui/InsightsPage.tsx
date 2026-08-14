import { useParams } from "react-router-dom";

import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { spellOut, useAsked, useEvery, useLive } from "@/shared/lib";
import { Empty, Figure, PageBody, Panel } from "@/shared/ui";
import { RepositoryHeader } from "@/widgets/repository-header";

export function InsightsPage() {
  const { owner = "", name = "", branch = "" } = useParams();
  const beat = useLive() + useEvery(60);
  const asked = useAsked(() => api.insights(owner, name, branch), [owner, name, branch], beat);

  return (
    <>
      <RepositoryHeader owner={owner} name={name} branch={branch} section="insights" />
      <WhatWeHave asked={asked} of="the numbers">
        {(said) => (
          <PageBody>
            <div className="space-y-6">
              <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
                <Figure label="Merged" value={String(said.merged)} />
                <Figure
                  label="Median to merge"
                  value={spellOut(said.median_wait_seconds)}
                  note="from asking to merged"
                />
                <Figure label="95th percentile" value={spellOut(said.p95_wait_seconds)} />
                <Figure
                  label="Left the queue"
                  value={String(said.failed + said.cancelled)}
                  note={`${said.failed} failed, ${said.cancelled} cancelled`}
                />
              </div>

              <Panel label="Merged per day">
                {said.merged_by_day.length === 0 ? (
                  <Empty>Nothing has merged through the queue yet.</Empty>
                ) : (
                  <ul className="space-y-2">
                    {said.merged_by_day.map((day) => (
                      <li key={day.day} className="flex items-center gap-3 text-ui">
                        <span className="w-24 shrink-0 font-mono text-mono text-ink-faint">
                          {day.day}
                        </span>
                        <Bar of={day.count} outOf={busiest(said.merged_by_day)} />
                        <span className="tabular-nums text-ink-faint">{day.count}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Panel>

              <Panel label="Why pull requests left the queue">
                {said.reasons.length === 0 ? (
                  <Empty>Nothing has been turned away.</Empty>
                ) : (
                  <ul className="space-y-2">
                    {said.reasons.map((reason) => (
                      <li
                        key={reason.reason}
                        className="flex items-baseline justify-between gap-4 text-ui"
                      >
                        <span className="min-w-0 flex-1 truncate">{reason.reason}</span>
                        <span className="shrink-0 tabular-nums text-ink-faint">{reason.count}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </Panel>
            </div>
          </PageBody>
        )}
      </WhatWeHave>
    </>
  );
}

function busiest(days: { count: number }[]): number {
  return Math.max(1, ...days.map((day) => day.count));
}

function Bar({ of, outOf }: { of: number; outOf: number }) {
  return (
    <span className="h-2 min-w-1 rounded-sm bg-bar" style={{ width: `${(of / outOf) * 100}%` }} />
  );
}
