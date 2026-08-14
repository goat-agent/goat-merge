import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";

import { useRepositories } from "@/entities/repository";
import { Select } from "@/shared/ui";

export function RepositoryHeader({
  owner,
  name,
  branch,
  section,
  right,
}: {
  owner: string;
  name: string;
  branch: string | null;
  section: "queue" | "history" | "insights" | "settings";
  right?: ReactNode;
}) {
  const navigate = useNavigate();
  const listed = useRepositories(0).answer ?? [];
  const here = `${owner}/${name}`;
  const branches = listed.find((one) => `${one.owner}/${one.name}` === here)?.queues ?? [];

  function goTo(repository: string, going: string | null) {
    const next = listed.find((one) => `${one.owner}/${one.name}` === repository);
    const first = next?.queues.at(0)?.branch ?? null;
    const on = going ?? first;
    navigate(
      on && section !== "settings" ? `/${section}/${repository}/${on}` : `/settings/${repository}`,
    );
  }

  return (
    <div className="flex h-header shrink-0 items-center justify-between gap-3 border-b border-hairline px-3">
      <div className="flex items-center gap-2">
        <Select
          label="Repository"
          value={here}
          options={listed.map((one) => `${one.owner}/${one.name}`)}
          onPick={(picked) => goTo(picked, null)}
        />
        {branch ? (
          <Select
            label="Branch"
            value={branch}
            options={branches.map((queue) => queue.branch)}
            onPick={(picked) => goTo(here, picked)}
          />
        ) : null}
      </div>
      {right}
    </div>
  );
}
