import { BarChart3, History, ListOrdered, Settings } from "lucide-react";
import type { ComponentType } from "react";
import { NavLink } from "react-router-dom";

import { cn } from "@/shared/lib";

type Section = {
  label: string;
  icon: ComponentType<{ className?: string }>;
  to: string | null;
};

function sections(owner: string | null, name: string | null, branch: string | null): Section[] {
  const inside = owner && name;
  const onBranch = inside && branch;
  return [
    {
      label: "Queue",
      icon: ListOrdered,
      to: onBranch ? `/queue/${owner}/${name}/${branch}` : null,
    },
    {
      label: "History",
      icon: History,
      to: onBranch ? `/history/${owner}/${name}/${branch}` : null,
    },
    {
      label: "Insights",
      icon: BarChart3,
      to: onBranch ? `/insights/${owner}/${name}/${branch}` : null,
    },
    { label: "Settings", icon: Settings, to: inside ? `/settings/${owner}/${name}` : null },
  ];
}

export function SideNav({
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
  return (
    <nav className="flex w-sidebar shrink-0 flex-col border-r border-hairline">
      <div className="flex h-header shrink-0 items-center px-4">
        <NavLink
          to="/"
          className="rounded-md font-mono text-ui font-semibold tracking-tight text-ink transition-opacity hover:opacity-80"
        >
          goat
          <span className="mx-0.5 text-ink-faint/50">/</span>
          <span className="text-ink-soft">merge</span>
        </NavLink>
      </div>

      <div className="min-h-0 flex-1 space-y-0.5 px-2">
        {sections(owner, name, branch).map((section) =>
          section.to ? (
            <NavLink
              key={section.label}
              to={section.to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2 rounded-md px-2 py-1.5 text-ui transition-colors",
                  isActive ? "font-medium text-primary" : "text-ink-faint hover:text-ink",
                )
              }
            >
              <section.icon className="size-3.5 shrink-0" />
              {section.label}
            </NavLink>
          ) : (
            <span
              key={section.label}
              title="Pick a repository first"
              className="flex items-center gap-2 rounded-md px-2 py-1.5 text-ui text-ink-faint opacity-40"
            >
              <section.icon className="size-3.5 shrink-0" />
              {section.label}
            </span>
          ),
        )}
      </div>

      <div className="shrink-0 truncate p-4 text-ui text-ink-faint">{login}</div>
    </nav>
  );
}
