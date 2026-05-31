import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/stores/data";
import { replayKeys } from "./queryKeys";

export function useReplayEvents() {
  return useQuery({
    queryKey: replayKeys.events(),
    queryFn: async () => {
      const resp = await apiFetch("/api/replay/events");
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return resp.json();
    },
    staleTime: 10 * 1000,
  });
}
