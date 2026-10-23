import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { replayKeys } from "./queryKeys"

export interface ReplayRelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

export interface ReplayEvent {
  id: number
  event_type: number
  event_timestamp: number
  good_or_bad: number
  target: string
  plate_name?: string
  stock_name?: string
  symbol?: string
  pcp?: number
  related_stocks?: ReplayRelatedStock[]
}

export interface ReplayIndexTick {
  tick_at: number
  close_px: number
}

export interface ReplayIndex {
  sh: ReplayIndexTick[]
  sz: ReplayIndexTick[]
  cyb: ReplayIndexTick[]
}

export interface ReplayDaySummary {
  date: string
  status: "partial" | "complete"
  event_count: number
  first_event_timestamp?: number
  last_event_timestamp?: number
  captured_at: string
  completed_at?: string
}

export interface ReplayDay extends ReplayDaySummary {
  returned_count: number
  events: ReplayEvent[]
  index: ReplayIndex | null
}

export function useReplayDays() {
  return useQuery({
    queryKey: replayKeys.days(),
    queryFn: () => fetchJson<ReplayDaySummary[]>("/api/replay/days?limit=30"),
    staleTime: 60 * 1000,
    refetchInterval: 60 * 1000,
  })
}

export function useReplayDay(date?: string) {
  return useQuery({
    queryKey: replayKeys.day(date),
    queryFn: () =>
      fetchJson<ReplayDay>(`/api/replay/day${date ? `?date=${encodeURIComponent(date)}` : ""}`),
    staleTime: 60 * 1000,
    refetchInterval: (query) => (query.state.data?.status === "partial" ? 60 * 1000 : false),
  })
}

export function useRefreshReplay() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (date: string) =>
      fetchJson<ReplayDaySummary>(`/api/replay/refresh?date=${encodeURIComponent(date)}`, {
        method: "POST",
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: replayKeys.all }),
  })
}
