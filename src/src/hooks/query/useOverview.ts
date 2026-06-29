import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { overviewKeys } from "./queryKeys"
import type { OverviewData } from "@/stores/data"

export function useOverview() {
  return useQuery({
    queryKey: overviewKeys.all,
    queryFn: () => fetchJson<OverviewData>("/api/overview"),
    staleTime: 2 * 60 * 1000,
  })
}
