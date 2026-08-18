import { CheckCircle2, Loader2, XCircle } from "lucide-react";
import type { ComponentType } from "react";

import type { Row, Trial } from "@/shared/api";
import { ago, cn } from "@/shared/lib";

type Look = {
  icon: ComponentType<{ className?: string }>;
  tint: string;
  turning: boolean;
  edge: string;
};

const running: Look = {
  icon: Loader2,
  tint: "text-ink-soft",
  turning: true,
  edge: "border-hairline",
};

const looks: Record<string, Look> = {
  pending: running,
  success: { icon: CheckCircle2, tint: "text-success", turning: false, edge: "border-success/40" },
  failure: { icon: XCircle, tint: "text-danger", turning: false, edge: "border-danger/40" },
  timed_out: { icon: XCircle, tint: "text-warning", turning: false, edge: "border-warning/40" },
};

function howMany(count: number): string {
  return count === 1 ? "1 pull request" : `${count} pull requests`;
}

function numbers(these: number[]): string {
  const written = these.map((one) => `#${one}`);
  if (written.length <= 1) return written.join("");
  return `${written.slice(0, -1).join(", ")} and ${written[written.length - 1]}`;
}

function headline(attempt: Trial, rows: Row[]): string {
  const merged = rows.length > 0 && rows.every((row) => row.status === "Merged");
  if (attempt.conclusion === "success") return merged ? "Merged" : "Passed";
  if (attempt.conclusion === "failure") return "Failed";
  if (attempt.conclusion === "timed_out") return "Ran out of time";
  return rows.some((row) => row.status === "Preparing") ? "Preparing" : "Checking";
}

function stillWaitingOn(assuming: number[], settled: boolean): string {
  if (settled) return `on top of ${numbers(assuming)}`;
  const one = assuming.length === 1;
  return `on top of ${numbers(assuming)}, which ${one ? "has" : "have"} not landed yet`;
}

function about(attempt: Trial, settled: boolean): string[] {
  const said = [`${howMany(attempt.aboard.length)} on 1 CI run`];
  if (attempt.narrowed_from !== null) said.push("narrowed down after a failure");
  said.push(
    attempt.assuming.length > 0
      ? stillWaitingOn(attempt.assuming, settled)
      : `on ${attempt.base.slice(0, 7)}`,
  );
  said.push(`${ago(attempt.finished_at ?? attempt.started_at)} ago`);
  return said;
}

function whatItMeans(attempt: Trial, rows: Row[]): string | null {
  const broke = attempt.conclusion === "failure" || attempt.conclusion === "timed_out";
  const landed = rows.length > 0 && rows.every((row) => row.status === "Merged");
  if (attempt.assuming.length > 0 && !landed) {
    const ahead = numbers(attempt.assuming);
    if (broke) {
      return `This was verified on top of ${ahead}, so its failure accuses nobody. It is thrown away and built again.`;
    }
    if (attempt.conclusion === "success") {
      return `This already passed. It merges the moment ${ahead} land, without running again.`;
    }
    return `This is running now, so it can merge the moment ${ahead} land.`;
  }
  if (rows.length < 2) return null;
  if (broke) return "They failed together, so none of them is the one at fault yet.";
  if (landed) return "One CI run landed all of them.";
  if (attempt.conclusion === "success") return null;
  return "These merge together or none of them do.";
}

export function BatchGroup({
  attempt,
  rows,
  owner,
  name,
  children,
}: {
  attempt: Trial;
  rows: Row[];
  owner: string;
  name: string;
  children: React.ReactNode;
}) {
  const look = looks[attempt.conclusion] ?? running;
  const Icon = look.icon;
  return (
    <section className={cn("mx-4 mb-3 rounded-md border", look.edge)}>
      <header className="flex flex-wrap items-baseline gap-x-2 gap-y-1 px-3 py-2 text-ui">
        <Icon
          className={cn(
            "size-3.5 shrink-0 self-center",
            look.tint,
            look.turning && "animate-spin motion-reduce:animate-none",
          )}
        />
        <span className="text-ink">{headline(attempt, rows)}</span>
        <span className="text-ink-faint">
          {about(
            attempt,
            rows.every((row) => row.settled_at !== null),
          ).join(" · ")}
        </span>
        <span className="flex-1" />
        {attempt.candidate_pull_request ? (
          <a
            className="shrink-0 text-primary underline-offset-2 hover:underline"
            href={`https://github.com/${owner}/${name}/pull/${attempt.candidate_pull_request}`}
            target="_blank"
            rel="noreferrer"
          >
            candidate #{attempt.candidate_pull_request}
          </a>
        ) : null}
      </header>
      {attempt.failed_checks.length > 0 ? (
        <p className="px-3 pb-2 text-ui text-danger">
          {attempt.failed_checks.map((check) => `"${check}"`).join(", ")} failed
        </p>
      ) : null}
      <ul className="border-t border-hairline">{children}</ul>
      {whatItMeans(attempt, rows) ? (
        <p className="px-3 py-2 text-ui text-ink-faint">{whatItMeans(attempt, rows)}</p>
      ) : null}
    </section>
  );
}
