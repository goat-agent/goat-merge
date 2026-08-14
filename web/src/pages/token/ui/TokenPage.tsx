import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";
import { Code, PageBody, Panel } from "@/shared/ui";

const commands = [
  "goat-merge queue",
  "goat-merge enqueue",
  "goat-merge explain",
  "goat-merge retry 123",
  "goat-merge queue pause",
  "goat-merge config validate",
].join("\n");

export function TokenPage() {
  const asked = useAsked(() => api.token(), []);

  return (
    <PageBody narrow>
      <div className="space-y-6">
        <h1 className="text-title text-ink">Use goat-merge from a terminal</h1>
        <WhatWeHave asked={asked} of="your token">
          {(said) => (
            <div className="space-y-6">
              <Panel label="Sign the CLI in" flush>
                <Code>{`goat-merge login ${said.url} --token ${said.token}`}</Code>
              </Panel>
              <p className="text-ui text-ink-faint">
                This is your own session. Signing out stops it working everywhere.
              </p>
              <Panel label="Then" flush>
                <Code>{commands}</Code>
              </Panel>
            </div>
          )}
        </WhatWeHave>
      </div>
    </PageBody>
  );
}
