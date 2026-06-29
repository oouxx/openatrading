import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { superStockKeys } from "./queryKeys"
import type { Stock } from "@/stores/data"

export function useSuperStock() {
  return useQuery({
    queryKey: superStockKeys.all,
    queryFn: () => fetchJson<Stock[]>("/api/super-stock"),
    staleTime: 30 * 1000,
  })
}
