import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/stores/data";
import { platesByStockKeys } from "./queryKeys";

export function usePlatesByStock(symbol: string) {
  return useQuery({
    queryKey: platesByStockKeys.bySymbol(symbol),
    queryFn: async () => {
      const resp = await apiFetch(`/api/plates-by-any-stock/${symbol}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return resp.json();
    },
    staleTime: 30 * 1000,
    enabled: !!symbol,
  });
}
