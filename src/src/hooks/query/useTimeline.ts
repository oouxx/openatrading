import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { timelineKeys } from "./queryKeys";

export function useTimeline() {
  return useQuery({
    queryKey: timelineKeys.all,
    queryFn: () => fetchJson<unknown[]>("/api/timeline"),
    staleTime: 6 * 60 * 60 * 1000,
  });
}
