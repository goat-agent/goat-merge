import { useState } from "react";

import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";
import { Button, Field, PageBody, Panel } from "@/shared/ui";

export function SetupPage() {
  const [org, setOrg] = useState("");
  const [appName, setAppName] = useState("Merge Queue");
  const asked = useAsked(() => api.setupManifest(), []);

  return (
    <PageBody narrow>
      <WhatWeHave asked={asked} of="the setup">
        {(said) => {
          const to = org.trim() ? said.organization.replace("ORG", org.trim()) : said.personal;
          const chosen = appName.replace(/["\\]/g, "").trim() || "Merge Queue";
          const manifest = said.manifest.replace("APP_NAME", chosen);
          return (
            <div className="space-y-6">
              <h1 className="text-display text-ink">Set up goat-merge</h1>

              {said.already_set_up ? (
                <Panel>
                  <p className="text-ui">
                    A GitHub App is already connected. Creating another one replaces it, and every
                    queue goes with it.
                  </p>
                </Panel>
              ) : null}

              <Panel label="1 · Create the GitHub App">
                <div className="space-y-4">
                  <p className="text-ui">
                    GitHub shows you a page with the name, permissions and webhook already filled
                    in. Nothing is stored here until you come back.
                  </p>
                  <p className="text-ui text-warning">
                    Sign in to GitHub first. The settings are posted from this page, and GitHub
                    drops them if it stops to ask you to sign in. If you land on an empty form, come
                    back and press the button again.
                  </p>
                  <div className="flex items-center gap-3 text-ui">
                    <label htmlFor="app-name" className="w-28 shrink-0 text-ink-faint">
                      App name
                    </label>
                    <Field
                      id="app-name"
                      className="flex-1"
                      value={appName}
                      onChange={(event) => setAppName(event.target.value)}
                    />
                  </div>
                  <p className="text-ui text-ink-faint">
                    GitHub App names are unique across the whole of GitHub, so a plain one is often
                    taken. If GitHub says so, pick another here or on its own page — the name in
                    pull requests and the config file does not change with it.
                  </p>
                  <div className="flex items-center gap-3 text-ui">
                    <label htmlFor="organization" className="w-28 shrink-0 text-ink-faint">
                      Organization
                    </label>
                    <Field
                      id="organization"
                      className="flex-1"
                      placeholder="leave empty for your own account"
                      value={org}
                      onChange={(event) => setOrg(event.target.value)}
                    />
                  </div>
                  <form action={to} method="post">
                    <input type="hidden" name="manifest" value={manifest} />
                    <Button type="submit" tone="primary">
                      Create the App on GitHub
                    </Button>
                  </form>
                </div>
              </Panel>

              <Panel label="2 · Install it on a repository">
                <p className="text-ui">
                  GitHub sends you to the install screen straight after the App is created. Pick the
                  repositories you want a queue on. Installing signs you in here too, so there is no
                  separate sign-in step.
                </p>
              </Panel>

              <Panel label="3 · Switch the queue on">
                <p className="text-ui">
                  You land back here on the repository you just installed. goat-merge checks the
                  branch and tells you what is missing before you turn the queue on.
                </p>
              </Panel>
            </div>
          );
        }}
      </WhatWeHave>
    </PageBody>
  );
}
