import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/stores/data";
import { realtimeKeys } from "./queryKeys";

export function useRealtimeQuote(symbol: string) {
  return useQuery({
    queryKey: realtimeKeys.bySymbol(symbol),
    queryFn: async () => {
      const resp = await apiFetch(`/api/realtime/${symbol}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return resp.json();
    },
    staleTime: 10 * 1000,
    enabled: !!symbol,
  });
}
