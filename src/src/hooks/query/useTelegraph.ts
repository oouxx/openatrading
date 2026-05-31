import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/stores/data";
import { telegraphKeys } from "./queryKeys";

export function useTelegraph() {
  return useQuery({
    queryKey: telegraphKeys.all,
    queryFn: async () => {
      const resp = await apiFetch("/api/telegraph");
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      return resp.json();
    },
    staleTime: 30 * 1000,
  });
}
