import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { indexQuotesKeys } from "./queryKeys";
import type { IndexQuote } from "@/stores/data";

export function useIndexQuotes() {
  return useQuery({
    queryKey: indexQuotesKeys.all,
    queryFn: () => fetchJson<IndexQuote[]>("/api/index-quotes"),
    staleTime: 10 * 1000,
  });
}
