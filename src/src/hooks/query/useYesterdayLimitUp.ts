import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { yesterdayLimitUpKeys } from "./queryKeys"
import type { Stock } from "@/stores/data"

export function useYesterdayLimitUp() {
  return useQuery({
    queryKey: yesterdayLimitUpKeys.all,
    queryFn: () => fetchJson<Stock[]>("/api/yesterday-limit-up"),
    staleTime: 30 * 1000,
  })
}
