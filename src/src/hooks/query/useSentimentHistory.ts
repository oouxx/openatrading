import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { sentimentHistoryKeys } from "./queryKeys"
import type { SentimentHistoryDay } from "@/stores/data"

export function useSentimentHistory(days = 10) {
  return useQuery({
    queryKey: sentimentHistoryKeys.byDays(days),
    queryFn: () => fetchJson<SentimentHistoryDay[]>(`/api/sentiment-history?days=${days}`),
    staleTime: 6 * 60 * 60 * 1000,
  })
}
