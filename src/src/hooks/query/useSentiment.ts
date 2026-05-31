import { useQuery } from "@tanstack/react-query";
import { fetchJson } from "@/stores/data";
import { sentimentKeys } from "./queryKeys";

export function useSentiment() {
  return useQuery({
    queryKey: sentimentKeys.all,
    queryFn: () => fetchJson<unknown>("/api/sentiment"),
    staleTime: 2 * 60 * 1000,
  });
}
