import { api } from "@/shared/api";
import { useAsked } from "@/shared/lib";

export function useViewer() {
  return useAsked<{ login: string }>(() => api.me(), []);
}
