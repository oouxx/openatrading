import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { limitUpKeys } from "./queryKeys"
import type { Stock } from "@/stores/data"

export function useLimitUp() {
  return useQuery({
    queryKey: limitUpKeys.all,
    queryFn: () => fetchJson<Stock[]>("/api/limit-up"),
    staleTime: 30 * 1000,
  })
}
