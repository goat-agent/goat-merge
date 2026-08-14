import { ExternalLink, MinusCircle, PlusCircle, RotateCcw, Zap } from "lucide-react";
import { useId, useState } from "react";

import { WhatWentWrong } from "@/entities/trouble";
import type { Row } from "@/shared/api";
import { api, Trouble } from "@/shared/api";
import { Button, Field, Well } from "@/shared/ui";

export function PullActions({
  owner,
  name,
  row,
  onChanged,
}: {
  owner: string;
  name: string;
  row: Row;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [wrong, setWrong] = useState<Trouble | null>(null);
  const [asking, setAsking] = useState(false);
  const [reason, setReason] = useState("");
  const why = useId();

  async function run(what: () => Promise<unknown>) {
    setBusy(true);
    setWrong(null);
    try {
      await what();
      onChanged();
    } catch (problem: unknown) {
      setWrong(
        problem instanceof Trouble
          ? problem
          : new Trouble("broken", "The console could not finish that.", null, null),
      );
    } finally {
      setBusy(false);
    }
  }

  const settled = row.settled_at !== null;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-1">
        {settled ? (
          <Button
            disabled={busy}
            onClick={() => run(() => api.act(owner, name, row.pull_request, "enqueue"))}
          >
            <PlusCircle className="size-3.5" />
            Queue again
          </Button>
        ) : (
          <Button
            disabled={busy}
            onClick={() => run(() => api.act(owner, name, row.pull_request, "dequeue"))}
          >
            <MinusCircle className="size-3.5" />
            Remove
          </Button>
        )}
        {row.status === "Failed" ? (
          <Button
            disabled={busy}
            onClick={() => run(() => api.act(owner, name, row.pull_request, "retry"))}
          >
            <RotateCcw className="size-3.5" />
            Retry
          </Button>
        ) : null}
        {settled ? null : (
          <Button
            disabled={busy}
            aria-expanded={asking}
            aria-controls={why}
            onClick={() => setAsking((was) => !was)}
          >
            <Zap className="size-3.5" />
            Expedite
          </Button>
        )}
        <Button
          href={`https://github.com/${owner}/${name}/pull/${row.pull_request}`}
          target="_blank"
          rel="noreferrer"
        >
          <ExternalLink className="size-3.5" />
          GitHub
        </Button>
      </div>

      {asking ? (
        <Well className="space-y-2" id={why}>
          <label className="block text-ui text-ink-faint" htmlFor={`${why}-reason`}>
            Expediting pushes everyone else down the queue, so the reason is written into the log.
          </label>
          <Field
            id={`${why}-reason`}
            className="w-full"
            placeholder="production incident"
            value={reason}
            onChange={(event) => setReason(event.target.value)}
          />
          <Button
            disabled={busy || reason.trim().length === 0}
            onClick={() =>
              run(async () => {
                await api.expedite(owner, name, row.pull_request, reason.trim());
                setAsking(false);
                setReason("");
              })
            }
          >
            Move to the front
          </Button>
        </Well>
      ) : null}

      {wrong ? <WhatWentWrong trouble={wrong} place="banner" /> : null}
    </div>
  );
}
