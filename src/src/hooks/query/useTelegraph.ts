import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { telegraphKeys } from "./queryKeys";

export function useTelegraph() {
  return useQuery({
    queryKey: telegraphKeys.all,
    queryFn: () => fetchJson("/api/telegraph"),
    staleTime: 30 * 1000,
  });
}
