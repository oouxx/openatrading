import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { platesByStockKeys } from "./queryKeys"

/** 个股所属板块（/api/plates-by-any-stock/:symbol）。 */
export interface PlateByStockItem {
  plate_id: number
  plate_name: string
  core_avg_pcp: number
  reason?: string
}

export function usePlatesByStock(symbol: string) {
  return useQuery({
    queryKey: platesByStockKeys.bySymbol(symbol),
    queryFn: () => fetchJson<PlateByStockItem[]>(`/api/plates-by-any-stock/${symbol}`),
    staleTime: 30 * 1000,
    enabled: !!symbol,
  })
}
