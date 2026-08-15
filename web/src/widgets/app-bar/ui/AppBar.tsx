import { Settings } from "lucide-react";
import { NavLink, useNavigate } from "react-router-dom";

import { itsQueue, whereItLives } from "@/entities/repository";
import { AccountMenu } from "@/features/account-menu";
import type { RepositoryView } from "@/shared/api";
import { cn } from "@/shared/lib";
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

const beside = "flex h-control shrink-0 items-center rounded-md px-2 text-ui transition-colors";

export function AppBar({
  owner,
  name,
  branch,
  login,
  repositories,
}: {
  owner: string | null;
  name: string | null;
  branch: string | null;
  login: string;
  repositories: RepositoryView[];
}) {
  const navigate = useNavigate();
  const here = owner && name ? `${owner}/${name}` : null;
  const looking = repositories.find((one) => `${one.owner}/${one.name}` === here);
  const itsOwnQueue = looking ? itsQueue(looking) : null;

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
              options={repositoryChoices(repositories, here)}
              onPick={(picked) => {
                const next = repositories.find((one) => `${one.owner}/${one.name}` === picked);
                navigate(next ? whereItLives(next) : `/settings/${picked}`);
              }}
            />
            <NavLink
              to={`/settings/${here}`}
              title={`Settings for ${here}`}
              aria-label={`Settings for ${here}`}
              className={({ isActive }) =>
                cn(
                  "flex size-control shrink-0 items-center justify-center rounded-md transition-colors",
                  isActive ? "text-primary" : "text-ink-faint hover:bg-fill-hover hover:text-ink",
                )
              }
            >
              <Settings className="size-3.5" />
            </NavLink>
            {branch ? (
              <Select
                label="Branch"
                value={branch}
                options={branchChoices(looking)}
                onPick={(picked) => navigate(`/queue/${here}/${picked}`)}
              />
            ) : itsOwnQueue ? (
              <NavLink
                to={itsOwnQueue}
                className={cn(beside, "text-ink-faint hover:bg-fill-hover hover:text-ink")}
              >
                Queue
              </NavLink>
            ) : null}
          </>
        ) : null}
      </div>
      <AccountMenu login={login} />
    </header>
  );
}
