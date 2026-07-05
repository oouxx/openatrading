import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { replayKeys } from "./queryKeys"

export function useReplayEvents(minPct = 2) {
  return useQuery({
    queryKey: replayKeys.events(minPct),
    queryFn: () => fetchJson(`/api/replay/events?min_pcp=${minPct / 100}`),
    staleTime: 10 * 1000,
  })
}
