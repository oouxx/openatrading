import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { RealtimeQuote } from "@/stores/data"
import { realtimeKeys } from "./queryKeys"

/** 单只股票的最新行情（现价、涨跌幅）。`symbol` 为空时不请求。 */
export function useRealtimeQuote(symbol: string) {
  return useQuery({
    queryKey: realtimeKeys.bySymbol(symbol),
    queryFn: () => fetchJson<RealtimeQuote>(`/api/realtime/${symbol}`),
    staleTime: 10 * 1000,
    enabled: !!symbol,
  })
}
