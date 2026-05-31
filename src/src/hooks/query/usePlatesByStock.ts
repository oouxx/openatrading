import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { platesByStockKeys } from "./queryKeys";

export function usePlatesByStock(symbol: string) {
  return useQuery({
    queryKey: platesByStockKeys.bySymbol(symbol),
    queryFn: () => fetchJson(`/api/plates-by-any-stock/${symbol}`) as Promise<any[]>,
    staleTime: 30 * 1000,
    enabled: !!symbol,
  });
}
