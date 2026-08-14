import type { Kind } from "./kinds";
import { isKind } from "./kinds";
import type { Read } from "./reading";
import { anything, list, maybe, need, shape, text } from "./reading";
import {
  readsAsACliToken,
  readsAsADiagnosis,
  readsAsAManifest,
  readsAsAQueue,
  readsAsARepository,
  readsAsARow,
  readsAsATimeline,
  readsAsAViewer,
  readsAsEnabled,
  readsAsHealth,
  readsAsInsights,
} from "./types";

export class Trouble extends Error {
  readonly kind: Kind;
  readonly goTo: string | null;
  readonly incident: string | null;

  constructor(kind: Kind, sentence: string, goTo: string | null, incident: string | null) {
    super(sentence);
    this.name = "Trouble";
    this.kind = kind;
    this.goTo = goTo;
    this.incident = incident;
  }
}

type Refusal = { error: string; kind: Kind; where: string | null; incident: string | null };

const readsAsARefusal: Read<Refusal> = shape((given) => ({
  error: need(text, given.error),
  kind: need((value) => (isKind(value) ? { got: value } : null), given.kind),
  where: need(maybe(text), given.where),
  incident: need(maybe(text), given.incident),
}));

const ourOwnWords: Record<"offline" | "unreadable", string> = {
  offline:
    "The console cannot reach goat-merge. It may be restarting, or your connection may have dropped.",
  unreadable:
    "Something between your browser and goat-merge answered instead of it, and the console cannot read the answer. If you are behind a proxy or a VPN sign-in page, that is usually the cause.",
};

const watching = new Set<() => void>();

export function whenTheSessionEnds(listener: () => void): () => void {
  watching.add(listener);
  return () => {
    watching.delete(listener);
  };
}

function raise(trouble: Trouble): never {
  if (trouble.kind === "not_signed_in") {
    for (const listener of watching) listener();
  }
  throw trouble;
}

async function ask<T>(path: string, init: RequestInit, reads: Read<T>): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, init);
  } catch {
    return raise(new Trouble("cannot_reach_the_server", ourOwnWords.offline, null, null));
  }

  const body = await response.text();
  const given: unknown = parsed(body);

  if (!response.ok) {
    const refusal = readsAsARefusal(given);
    if (refusal) {
      const { error, kind, where, incident } = refusal.got;
      return raise(new Trouble(kind, error, where, incident));
    }
    return raise(new Trouble("answer_was_not_json", ourOwnWords.unreadable, null, null));
  }

  const answer = reads(given);
  if (!answer) {
    return raise(new Trouble("answer_was_not_json", ourOwnWords.unreadable, null, null));
  }
  return answer.got;
}

function parsed(body: string): unknown {
  try {
    return JSON.parse(body);
  } catch {
    return undefined;
  }
}

function get<T>(path: string, reads: Read<T>) {
  return ask(path, { method: "GET" }, reads);
}

function post<T>(path: string, reads: Read<T>, body?: unknown) {
  return ask(
    path,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body ?? {}),
    },
    reads,
  );
}

export const api = {
  health: () => get("/api/health", readsAsHealth),
  me: () => get("/api/me", readsAsAViewer),
  token: () => get("/api/token", readsAsACliToken),
  logout: () => post("/auth/logout", anything),

  repositories: () => get("/api/repositories", list(readsAsARepository)),
  diagnose: (owner: string, name: string, branch?: string) =>
    get(
      `/api/repository/${owner}/${name}/diagnose${branch ? `?branch=${branch}` : ""}`,
      readsAsADiagnosis,
    ),
  enable: (owner: string, name: string, how: Record<string, unknown>) =>
    post(`/api/repository/${owner}/${name}/enable`, readsAsEnabled, how),
  disable: (owner: string, name: string) =>
    post(`/api/repository/${owner}/${name}/disable`, anything),

  queue: (owner: string, name: string, branch: string) =>
    get(`/api/queue/${owner}/${name}/${branch}`, readsAsAQueue),
  history: (owner: string, name: string, branch: string) =>
    get(`/api/history/${owner}/${name}/${branch}`, list(readsAsARow)),
  insights: (owner: string, name: string, branch: string) =>
    get(`/api/insights/${owner}/${name}/${branch}`, readsAsInsights),
  pause: (owner: string, name: string, branch: string) =>
    post(`/api/queue/${owner}/${name}/${branch}/pause`, anything),
  resume: (owner: string, name: string, branch: string) =>
    post(`/api/queue/${owner}/${name}/${branch}/resume`, anything),

  pull: (owner: string, name: string, number: number) =>
    get(`/api/pull/${owner}/${name}/${number}`, readsAsATimeline),
  act: (owner: string, name: string, number: number, what: string) =>
    post(`/api/pull/${owner}/${name}/${number}/${what}`, anything),
  expedite: (owner: string, name: string, number: number, reason: string) =>
    post(`/api/pull/${owner}/${name}/${number}/expedite`, anything, { reason }),

  setupManifest: () => get("/api/setup/manifest", readsAsAManifest),
};
