import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import type { ComponentType } from "react";
import { useState } from "react";
import { useParams } from "react-router-dom";

import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import type { Advice } from "@/shared/api";
import { api, Trouble } from "@/shared/api";
import { cn, useAsked, useEvery, useLive } from "@/shared/lib";
import { Badge, Button, Code, Empty, PageBody, Panel, Select } from "@/shared/ui";

export function SettingsPage() {
  const { owner = "", name = "" } = useParams();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [wrong, setWrong] = useState<Trouble | null>(null);
  const [method, setMethod] = useState("");
  const asked = useAsked(() => api.diagnose(owner, name), [owner, name], useLive() + useEvery(30));

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
        write_config: withConfig,
      });
      return done.config_pull_request
        ? `The queue is on for ${done.branch}, and pull request #${done.config_pull_request} adds the configuration file.`
        : `The queue is on for ${done.branch}.`;
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
                  settled={found.merge_methods.length === 1 || found.config !== null}
                  says={`merge methods allowed: ${found.merge_methods.join(", ") || "none"}`}
                />
                <Line
                  settled={found.label.exists}
                  says={`the ${found.label.name} label ${found.label.exists ? "exists" : "does not exist yet"}`}
                />
                <Line
                  settled={found.fork_workflow}
                  says={
                    found.fork_workflow
                      ? "a fork queue workflow is declared, so fork pull requests can be verified safely"
                      : "no fork queue workflow, so pull requests from forks will be blocked"
                  }
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
                <div className="flex flex-wrap gap-2">
                  <Button tone="primary" disabled={busy} onClick={() => enable(found.branch, true)}>
                    Enable and open a configuration pull request
                  </Button>
                  <Button disabled={busy} onClick={() => enable(found.branch, false)}>
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

const whateverIsAllowed = "whatever the repository allows";

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
      <span>{advice.text}</span>
    </li>
  );
}
