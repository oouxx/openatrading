import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { replayKeys } from "./queryKeys"

export function useReplayEvents() {
  return useQuery({
    queryKey: replayKeys.events(),
    queryFn: () => fetchJson("/api/replay/events"),
    staleTime: 10 * 1000,
  })
}
