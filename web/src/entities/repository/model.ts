import type { RepositoryView } from "@/shared/api";
import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";

export function useRepositories(beat: number) {
  return useAsked<RepositoryView[]>(() => api.repositories(), [], beat);
}

export function itsQueue(repository: RepositoryView): string | null {
  const queue = repository.queues.at(0);
  if (!queue) return null;
  return `/queue/${repository.owner}/${repository.name}/${queue.branch}`;
}

export function whereItLives(repository: RepositoryView): string {
  return itsQueue(repository) ?? `/settings/${repository.owner}/${repository.name}`;
}

export function whereToStart(repositories: RepositoryView[]): string | null {
  const running = repositories.find((one) => one.active && itsQueue(one) !== null);
  const anywhere = running ?? repositories.at(0);
  return anywhere ? whereItLives(anywhere) : null;
}
