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
    // 买入弹窗停留时现价也要继续走，避免用户按几分钟前的价格成交。
    refetchInterval: 15_000,
    refetchOnWindowFocus: true,
    enabled: !!symbol,
  })
}
