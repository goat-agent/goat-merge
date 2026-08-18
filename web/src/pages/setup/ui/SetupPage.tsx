import { WhatWeHave } from "@/entities/trouble";
import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";
import { Button, PageBody, Panel } from "@/shared/ui";

export function SetupPage() {
  const asked = useAsked(() => api.setupManifest(), []);

  return (
    <PageBody narrow>
      <WhatWeHave asked={asked} of="the setup">
        {(said) => (
          <div className="space-y-6">
            <h1 className="text-display text-ink">Set up goat-merge</h1>

            {said.already_set_up ? (
              <Panel>
                <p className="text-ui">
                  A GitHub App is already connected. Creating another one replaces it, and every
                  queue goes with it. To put the queue on more repositories, install the App you
                  already have instead.
                </p>
                <p className="pt-3 text-ui">
                  <a
                    href="/setup/install"
                    className="text-primary underline-offset-2 hover:underline"
                  >
                    Install it somewhere else
                  </a>
                </p>
              </Panel>
            ) : null}

            <Panel label="1 · Create the GitHub App">
              <div className="space-y-4">
                <p className="text-ui">
                  GitHub shows you a page with the permissions and webhook already filled in. You
                  give it a name there. Nothing is stored here until you come back.
                </p>
                <p className="text-ui text-warning">
                  Sign in to GitHub first. The settings are posted from this page, and GitHub drops
                  them if it stops to ask you to sign in. If you land on an empty form, come back
                  and press the button again.
                </p>
                <form action={said.where} method="post">
                  <input type="hidden" name="manifest" value={said.manifest} />
                  <Button type="submit" tone="primary">
                    Create the App on GitHub
                  </Button>
                </form>
              </div>
            </Panel>

            <Panel label="2 · Install it where the repositories are">
              <p className="text-ui">
                GitHub sends you to the install screen straight after the App is created. Pick the
                account — yours or an organisation — and the repositories you want a queue on.
                Installing signs you in here too, so there is no separate sign-in step.
              </p>
            </Panel>

            <Panel label="3 · Switch the queue on">
              <p className="text-ui">
                You land back here on the repository you just installed. goat-merge checks the
                branch and tells you what is missing before you turn the queue on.
              </p>
            </Panel>
          </div>
        )}
      </WhatWeHave>
    </PageBody>
  );
}
