import { Settings } from "lucide-react";
import { NavLink, useNavigate } from "react-router-dom";

import { useRepositories } from "@/entities/repository";
import { AccountMenu } from "@/features/account-menu";
import type { RepositoryView } from "@/shared/api";
import type { Choice } from "@/shared/ui";
import { Select } from "@/shared/ui";

function howItIsGoing(repository: RepositoryView): string {
  if (!repository.active) return "queue is off";
  if (repository.queues.length === 0) return "no branch queued";
  const paused = repository.queues.filter((queue) => queue.paused).length;
  if (paused === repository.queues.length) return "paused";
  if (paused > 0) return `${paused} paused`;
  return "";
}

function repositoryChoices(listed: RepositoryView[], here: string): Choice[] {
  return listed.map((one) => {
    const where = `${one.owner}/${one.name}`;
    const going = where === here ? "" : howItIsGoing(one);
    return { value: where, label: going ? `${where} — ${going}` : where };
  });
}

function branchChoices(repository: RepositoryView | undefined): Choice[] {
  return (repository?.queues ?? []).map((queue) => ({
    value: queue.branch,
    label: queue.paused ? `${queue.branch} — paused` : queue.branch,
  }));
}

export function AppBar({
  owner,
  name,
  branch,
  login,
}: {
  owner: string | null;
  name: string | null;
  branch: string | null;
  login: string;
}) {
  const navigate = useNavigate();
  const listed = useRepositories(0).answer ?? [];
  const here = owner && name ? `${owner}/${name}` : null;
  const looking = listed.find((one) => `${one.owner}/${one.name}` === here);

  function goTo(repository: string, going: string | null) {
    const next = listed.find((one) => `${one.owner}/${one.name}` === repository);
    const on = going ?? next?.queues.at(0)?.branch ?? null;
    navigate(on ? `/queue/${repository}/${on}` : `/settings/${repository}`);
  }

  return (
    <header className="flex h-header shrink-0 items-center justify-between gap-3 border-b border-hairline px-3">
      <div className="flex min-w-0 items-center gap-2">
        <NavLink
          to="/"
          className="mr-1 shrink-0 rounded-md font-mono text-ui font-semibold tracking-tight text-ink transition-opacity hover:opacity-80"
        >
          goat
          <span className="mx-0.5 text-ink-faint/50">/</span>
          <span className="text-ink-soft">merge</span>
        </NavLink>
        {here ? (
          <>
            <Select
              label="Repository"
              value={here}
              options={repositoryChoices(listed, here)}
              onPick={(picked) => goTo(picked, null)}
            />
            <NavLink
              to={`/settings/${here}`}
              title={`Settings for ${here}`}
              aria-label={`Settings for ${here}`}
              className={({ isActive }) =>
                [
                  "flex size-control shrink-0 items-center justify-center rounded-md transition-colors",
                  isActive ? "text-primary" : "text-ink-faint hover:bg-fill-hover hover:text-ink",
                ].join(" ")
              }
            >
              <Settings className="size-3.5" />
            </NavLink>
            {branch ? (
              <Select
                label="Branch"
                value={branch}
                options={branchChoices(looking)}
                onPick={(picked) => goTo(here, picked)}
              />
            ) : (
              <button
                type="button"
                onClick={() => goTo(here, null)}
                className="flex h-control shrink-0 items-center rounded-md px-2 text-ui text-ink-faint transition-colors hover:bg-fill-hover hover:text-ink"
              >
                Back to the queue
              </button>
            )}
          </>
        ) : null}
      </div>
      <AccountMenu login={login} />
    </header>
  );
}
