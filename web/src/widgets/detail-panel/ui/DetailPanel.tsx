import type { ReactNode } from "react";
import { useState } from "react";

import { Standing } from "@/entities/queue-entry";
import { WhatWeHave } from "@/entities/trouble";
import { PullActions } from "@/features/pull-actions";
import type { QueueView, Row, Timeline, Trial } from "@/shared/api";
import { api } from "@/shared/api";
import { ago, cn, useAsked } from "@/shared/lib";
import { Empty, Well } from "@/shared/ui";

type Tab = "detail" | "activity";

export function DetailPanel({
  owner,
  name,
  number,
  queue,
  beat,
  onChanged,
}: {
  owner: string;
  name: string;
  number: number | null;
  queue: QueueView | null;
  beat: number;
  onChanged: () => void;
}) {
  const [tab, setTab] = useState<Tab>("detail");
  const asked = useAsked<Timeline | null>(
    () => (number === null ? Promise.resolve(null) : api.pull(owner, name, number)),
    [owner, name, number],
    beat,
  );

  if (number === null) {
    return (
      <Beside>
        <div className="flex h-header shrink-0 items-center px-4">
          <h2 className="text-caption uppercase text-ink-faint">This queue</h2>
        </div>
        {queue ? (
          <HowItStands queue={queue} />
        ) : (
          <Empty>Pick a pull request to see why it is where it is.</Empty>
        )}
      </Beside>
    );
  }

  return (
    <Beside>
      <div className="flex h-header shrink-0 items-center gap-1 px-3" role="tablist">
        {(["detail", "activity"] as const).map((which) => (
          <button
            key={which}
            type="button"
            role="tab"
            aria-selected={tab === which}
            onClick={() => setTab(which)}
            className={cn(
              "rounded-md px-3 py-1.5 text-ui capitalize transition-colors",
              tab === which
                ? "bg-fill-subtle text-ink"
                : "text-ink-faint hover:bg-fill-hover hover:text-ink",
            )}
          >
            {which}
          </button>
        ))}
      </div>

      <WhatWeHave asked={asked} of="this pull request">
        {(said) =>
          said === null ? null : (
            <div className="min-h-0 flex-1 space-y-6 overflow-y-auto p-4">
              {tab === "detail" ? (
                <>
                  <div className="space-y-1">
                    <p className="font-mono text-mono text-ink-faint">#{said.entry.pull_request}</p>
                    <h2 className="text-heading text-ink">{said.entry.title || "untitled"}</h2>
                    <p className="text-ui text-ink-faint">asked by {said.entry.requested_by}</p>
                  </div>

                  <Well>
                    <Standing status={said.entry.status} />
                    <p className="text-ui">{said.entry.detail}</p>
                    {said.entry.expedite_note ? (
                      <p className="text-ui text-warning">
                        expedited by {said.entry.expedited_by}: {said.entry.expedite_note}
                      </p>
                    ) : null}
                  </Well>

                  <PullActions owner={owner} name={name} row={said.entry} onChanged={onChanged} />

                  {said.attempts.length > 0 ? (
                    <div className="space-y-2">
                      <h3 className="text-caption uppercase text-ink-faint">Verifications</h3>
                      {said.attempts
                        .slice()
                        .reverse()
                        .map((attempt) => (
                          <Attempt
                            key={`${attempt.candidate_branch}-${attempt.started_at}`}
                            attempt={attempt}
                            owner={owner}
                            name={name}
                          />
                        ))}
                    </div>
                  ) : null}
                </>
              ) : (
                <ul className="space-y-3">
                  {said.timeline.length === 0 ? <Empty>Nothing has happened yet.</Empty> : null}
                  {said.timeline.map((note) => (
                    <li key={`${note.at}-${note.action}`} className="text-ui">
                      <p>
                        <span className="text-ink">{note.actor}</span>{" "}
                        <span className="text-ink-faint">{note.action}</span>
                      </p>
                      {note.detail ? <p className="text-ink-faint">{note.detail}</p> : null}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )
        }
      </WhatWeHave>
    </Beside>
  );
}

function HowItStands({ queue }: { queue: QueueView }) {
  const running = queue.entries.filter((row: Row) => row.settled_at === null);
  const head = running.at(0);
  const blocked = running.filter((row: Row) => row.status === "Blocked");

  return (
    <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4 text-ui">
      <p className="text-ink">
        {running.length === 0
          ? "Nothing is in this queue."
          : `${running.length} pull ${running.length === 1 ? "request is" : "requests are"} in this queue.`}
      </p>

      {head ? (
        <Well>
          <p className="font-mono text-mono text-ink-faint">#{head.pull_request}</p>
          <Standing status={head.status} />
          <p className="text-ink-faint">{head.detail}</p>
          <p className="text-ink-faint">waiting {ago(head.requested_at)}</p>
        </Well>
      ) : null}

      {queue.paused ? (
        <p className="text-warning">
          The queue is paused by {queue.paused_by ?? "someone"}, so nothing will merge until it is
          resumed.
        </p>
      ) : null}

      {blocked.length > 0 ? (
        <p className="text-ink-faint">
          {blocked.length} {blocked.length === 1 ? "is" : "are"} blocked and will not move until
          somebody sees to them.
        </p>
      ) : null}

      {running.length > 0 ? (
        <p className="text-ink-faint">
          Pick a pull request to see which base it was verified against.
        </p>
      ) : null}
    </div>
  );
}

function Beside({ children }: { children: ReactNode }) {
  return (
    <aside
      aria-label="Pull request detail"
      className="flex w-detail shrink-0 flex-col border-l border-hairline"
    >
      {children}
    </aside>
  );
}

const conclusions: Record<string, string> = {
  success: "text-success",
  failure: "text-danger",
  timed_out: "text-warning",
  pending: "text-ink-soft",
};

function Attempt({ attempt, owner, name }: { attempt: Trial; owner: string; name: string }) {
  return (
    <Well className="text-ui">
      <div className="flex items-center justify-between">
        <span className="font-mono text-mono text-ink-faint">
          {attempt.aboard.map((number) => `#${number}`).join(", ")} on{" "}
          {attempt.base.slice(0, 7)}
        </span>
        <span className={conclusions[attempt.conclusion] ?? "text-ink-faint"}>
          {attempt.conclusion}
        </span>
      </div>
      {attempt.candidate_pull_request ? (
        <a
          className="text-primary underline-offset-2 hover:underline"
          href={`https://github.com/${owner}/${name}/pull/${attempt.candidate_pull_request}`}
          target="_blank"
          rel="noreferrer"
        >
          candidate #{attempt.candidate_pull_request}
        </a>
      ) : null}
      {attempt.failed_checks.map((check) => (
        <p key={check} className="text-danger">
          {check}
        </p>
      ))}
      <p className="text-ink-faint">started {ago(attempt.started_at)} ago</p>
    </Well>
  );
}
