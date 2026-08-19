import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import type { ComponentType } from "react";
import { useState } from "react";
import { useParams } from "react-router-dom";

import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import type { Advice, Diagnosis, Enabled } from "@/shared/api";
import { api, Trouble } from "@/shared/api";
import { cn, useAsked, useEvery, useLive } from "@/shared/lib";
import { Badge, Button, Code, Empty, PageBody, Panel, Select } from "@/shared/ui";

export function SettingsPage() {
  const { owner = "", name = "" } = useParams();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [wrong, setWrong] = useState<Trouble | null>(null);
  const [method, setMethod] = useState("");
  const [atOnce, setAtOnce] = useState(defaultBatchSize);
  const asked = useAsked(() => api.diagnose(owner, name), [owner, name], useLive() + useEvery(30));
  const allowed = asked.answer?.merge_methods ?? [];
  const mustPickAMergeMethod = allowed.length > 1 && method === "";

  async function doing(work: () => Promise<string | null>) {
    setBusy(true);
    setSaid(null);
    setWrong(null);
    try {
      setSaid(await work());
      asked.again();
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

  const enable = (branch: string, withConfig: boolean) =>
    doing(async () => {
      const done = await api.enable(owner, name, {
        branch,
        merge_method: method === "" ? null : method,
        batch_size: atOnce,
        write_config: withConfig,
      });
      return `The queue is on for ${done.branch}.${whatBecameOfTheFile(done)}`;
    });

  return (
    <WhatWeHave asked={asked} of="this repository">
      {(found) => (
        <PageBody narrow>
          <div className="space-y-6">
            <header className="flex items-baseline justify-between gap-3">
              <h1 className="text-title text-ink">
                {found.owner}/{found.name}
                <span className="ml-2 font-mono text-mono text-ink-faint">{found.branch}</span>
              </h1>
              {found.active ? (
                <Badge tone="success">queue is on</Badge>
              ) : (
                <Badge>queue is off</Badge>
              )}
            </header>

            <Panel label="Diagnosis">
              <ul className="space-y-2">
                <Line
                  settled={found.protection.declared}
                  says={
                    found.protection.declared
                      ? `${found.protection.required_checks.length} required checks, ${found.protection.required_approvals} approvals`
                      : "no ruleset or branch protection on this branch"
                  }
                />
                <Line
                  settled={found.enforced}
                  says={
                    found.enforced
                      ? `${found.check_name} is a required check, so nothing merges around the queue`
                      : `${found.check_name} is not required yet, so this branch can still be merged by hand`
                  }
                />
                <Line
                  settled={found.knows_how_to_merge}
                  says={`merge methods allowed: ${found.merge_methods.join(", ") || "none"}`}
                />
                <Line
                  settled={found.label.exists}
                  says={`the ${found.label.name} label ${found.label.exists ? "exists" : "does not exist yet"}`}
                />
                <Line
                  settled={found.fork_workflow === "safe"}
                  says={howForkPullRequestsStand[found.fork_workflow]}
                />
              </ul>
            </Panel>

            {found.advice.length > 0 ? (
              <Panel label="What to do next">
                <ul className="space-y-2">
                  {found.advice.map((advice) => (
                    <Suggestion key={advice.text} advice={advice} />
                  ))}
                </ul>
              </Panel>
            ) : null}

            <Panel label="Turn the queue on">
              <div className="space-y-4">
                <div className="flex items-center gap-3 text-ui">
                  <span className="text-ink-faint">Merge with</span>
                  <Select
                    label="Merge method"
                    value={method === "" ? whateverIsAllowed : method}
                    options={[whateverIsAllowed, ...found.merge_methods].map((one) => ({
                      value: one,
                      label: one,
                    }))}
                    onPick={(picked) => setMethod(picked === whateverIsAllowed ? "" : picked)}
                  />
                </div>
                <div className="flex items-center gap-3 text-ui">
                  <span className="text-ink-faint">Verify at most</span>
                  <Select
                    label="Pull requests on one candidate"
                    value={String(atOnce)}
                    options={batchSizes.map((one) => ({
                      value: String(one),
                      label: one === 1 ? "1 pull request" : `${one} pull requests`,
                    }))}
                    onPick={(picked) => setAtOnce(Number(picked) || defaultBatchSize)}
                  />
                  <span className="text-ink-faint">on one candidate</span>
                </div>
                <p className="text-ui text-ink-faint">
                  A ceiling, not a target. The queue starts at one and works up to it as the
                  repository proves itself, and halves back down when a batch fails until it finds
                  the pull request at fault.
                </p>
                {mustPickAMergeMethod ? (
                  <p className="text-ui text-warning">
                    This repository allows {allowed.join(", ")}. Pick one before turning the queue
                    on — a queue that has not been told which history you want blocks every pull
                    request rather than guess.
                  </p>
                ) : null}
                <div className="flex flex-wrap gap-2">
                  <Button
                    tone="primary"
                    disabled={busy || mustPickAMergeMethod}
                    onClick={() => enable(found.branch, true)}
                  >
                    Enable and open a configuration pull request
                  </Button>
                  <Button
                    disabled={busy || mustPickAMergeMethod}
                    onClick={() => enable(found.branch, false)}
                  >
                    Enable without a configuration file
                  </Button>
                  {found.active ? (
                    <Button
                      tone="danger"
                      disabled={busy}
                      onClick={() =>
                        doing(async () => {
                          await api.disable(owner, name);
                          return `The queue is off for ${found.owner}/${found.name}.`;
                        })
                      }
                    >
                      Turn it off
                    </Button>
                  ) : null}
                </div>
                {said ? <p className="text-ui">{said}</p> : null}
                {wrong ? <WhatWentWrong trouble={wrong} place="banner" /> : null}
              </div>
            </Panel>

            <section className="space-y-2">
              <h2 className="text-caption uppercase text-ink-faint">Configuration</h2>
              {found.config ? (
                <Code>{found.config}</Code>
              ) : (
                <div className="rounded-lg bg-sunken">
                  <Empty>
                    There is no <span className="font-mono">.github/merge-queue.yml</span>, so the
                    queue follows the repository's own rules.
                  </Empty>
                </div>
              )}
            </section>
          </div>
        </PageBody>
      )}
    </WhatWeHave>
  );
}

const howForkPullRequestsStand: Record<Diagnosis["fork_workflow"], string> = {
  safe: "a fork queue workflow is declared, so fork pull requests can be verified safely",
  refused:
    "a fork queue workflow is declared, but it asks for something a stranger's code must not have, so fork pull requests are still blocked",
  missing: "no fork queue workflow, so pull requests from forks will be blocked",
};

function whatBecameOfTheFile(done: Enabled): string {
  if (done.configuration === "opened") {
    return ` Pull request #${done.config_pull_request} adds the configuration file.`;
  }
  if (done.configuration === "already_says_that") {
    return ` ${configFile} already says that, so there was nothing to write.`;
  }
  if (done.configuration === "yours_to_edit") {
    return ` ${configFile} is already there and says something else, so it was left alone — nothing here rewrites a file somebody else wrote. Add this to its queue for ${done.branch}: ${(done.what_to_add ?? "").trim()}`;
  }
  return "";
}

const configFile = ".github/merge-queue.yml";

const whateverIsAllowed = "whatever the repository allows";
const batchSizes = [1, 2, 3, 5, 8, 10];
const defaultBatchSize = 5;

function Line({ settled, says }: { settled: boolean; says: string }) {
  const Icon = settled ? CheckCircle2 : AlertTriangle;
  return (
    <li className="flex items-start gap-2 text-ui">
      <Icon className={cn("mt-0.5 size-3.5 shrink-0", settled ? "text-success" : "text-warning")} />
      <span>{says}</span>
    </li>
  );
}

const marks: Record<
  Advice["level"],
  { icon: ComponentType<{ className?: string }>; tint: string }
> = {
  error: { icon: XCircle, tint: "text-danger" },
  warning: { icon: AlertTriangle, tint: "text-warning" },
  info: { icon: Info, tint: "text-ink-faint" },
};

function Suggestion({ advice }: { advice: Advice }) {
  const mark = marks[advice.level];
  const Icon = mark.icon;
  return (
    <li className="flex items-start gap-2 text-ui">
      <Icon className={cn("mt-0.5 size-3.5 shrink-0", mark.tint)} />
      <span>
        {advice.text}
        {advice.where && advice.follow ? (
          <>
            {" "}
            <a
              className="text-primary underline-offset-2 hover:underline"
              href={advice.where}
              target="_blank"
              rel="noreferrer"
            >
              {advice.follow}
            </a>
          </>
        ) : null}
      </span>
    </li>
  );
}
