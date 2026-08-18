import { useEffect, useState } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";

import { useRepositories, whereToStart } from "@/entities/repository";
import { WhatWeHave, WhatWentWrong } from "@/entities/trouble";
import { useViewer } from "@/entities/viewer";
import { HistoryPage } from "@/pages/history";
import { QueuePage } from "@/pages/queue";
import { SettingsPage } from "@/pages/settings";
import { SetupPage } from "@/pages/setup";
import { SignInPage } from "@/pages/sign-in";
import { TokenPage } from "@/pages/token";
import type { RepositoryView } from "@/shared/api";
import { api, isKind, Trouble, whenTheSessionEnds } from "@/shared/api";
import type { Asked } from "@/shared/lib";
import { useAsked, useEvery, useLive } from "@/shared/lib";
import { Waiting } from "@/shared/ui";
import { AppBar } from "@/widgets/app-bar";

import { whereWeAre } from "./whereWeAre";

export function App() {
  const health = useAsked(() => api.health(), []);
  const viewer = useViewer();
  const [ended, setEnded] = useState<Trouble | null>(null);
  const cameBack = troubleInTheAddress(useLocation().search);

  useEffect(() => whenTheSessionEnds(() => setEnded(sessionEnded())), []);

  if (health.firstLoad || viewer.firstLoad) {
    return <Waiting of="goat-merge" />;
  }

  if (!viewer.answer) {
    if (health.trouble && health.trouble.kind !== "not_signed_in") {
      return <WhatWentWrong trouble={health.trouble} onAgain={health.again} />;
    }
    return (
      <Routes>
        <Route path="/setup" element={<SetupPage />} />
        <Route
          path="*"
          element={<SignInPage setUp={health.answer?.set_up ?? false} trouble={cameBack} />}
        />
      </Routes>
    );
  }

  if (ended) {
    return <WhatWentWrong trouble={ended} />;
  }

  return <Console login={viewer.answer.login} />;
}

function sessionEnded(): Trouble {
  return new Trouble(
    "not_signed_in",
    "Your session has ended. Sign in with GitHub again and you will come straight back here.",
    "/auth/github",
    null,
  );
}

function troubleInTheAddress(search: string): Trouble | null {
  const asked = new URLSearchParams(search);
  const kind = asked.get("trouble");
  const said = asked.get("said");
  if (!isKind(kind) || !said) return null;
  return new Trouble(kind, said, null, null);
}

function Console({ login }: { login: string }) {
  const repositories = useRepositories(useLive() + useEvery(30));
  const here = whereWeAre(useLocation().pathname);

  return (
    <div className="flex h-full flex-col">
      <AppBar
        owner={here.owner}
        name={here.name}
        branch={here.branch}
        login={login}
        repositories={repositories.answer ?? []}
      />
      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        <Routes>
          <Route path="/queue/:owner/:name/:branch" element={<QueuePage />} />
          <Route path="/history/:owner/:name/:branch" element={<HistoryPage />} />
          <Route path="/settings/:owner/:name" element={<SettingsPage />} />
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/token" element={<TokenPage />} />
          <Route path="*" element={<Landing asked={repositories} />} />
        </Routes>
      </main>
    </div>
  );
}

function Landing({ asked }: { asked: Asked<RepositoryView[]> }) {
  return (
    <WhatWeHave asked={asked} of="your repositories">
      {(repositories) => {
        const start = whereToStart(repositories);
        if (start) return <Navigate to={start} replace />;
        return (
          <div className="flex flex-1 items-center justify-center p-6 text-center">
            <p className="max-w-measure text-ui text-ink-faint">
              The App is not installed on any repository yet. Install it on GitHub and come back.
            </p>
          </div>
        );
      }}
    </WhatWeHave>
  );
}
