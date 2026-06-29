import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { realtimeKeys } from "./queryKeys"

export function useRealtimeQuote(symbol: string) {
  return useQuery({
    queryKey: realtimeKeys.bySymbol(symbol),
    queryFn: () => fetchJson(`/api/realtime/${symbol}`) as Promise<any>,
    staleTime: 10 * 1000,
    enabled: !!symbol,
  })
}
