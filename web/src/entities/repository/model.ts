import type { RepositoryView } from "@/shared/api";
import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";

export function useRepositories(beat: number) {
  return useAsked<RepositoryView[]>(() => api.repositories(), [], beat);
}
