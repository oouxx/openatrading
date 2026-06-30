import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { watchlistKeys } from "./queryKeys"

export interface WatchlistItem {
  code: string
  name: string
  added_at: string
}

export function useWatchlist() {
  return useQuery({
    queryKey: watchlistKeys.all,
    queryFn: () => fetchJson<WatchlistItem[]>("/api/watchlist"),
    staleTime: 10 * 1000,
  })
}

export function useAddWatchlist() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (item: { code: string; name: string }) =>
      fetchJson<WatchlistItem[]>("/api/watchlist", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(item),
      }),
    onSettled: () => qc.invalidateQueries({ queryKey: watchlistKeys.all }),
  })
}

export function useDeleteWatchlist() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (code: string) =>
      fetchJson<WatchlistItem[]>(`/api/watchlist/${encodeURIComponent(code)}`, {
        method: "DELETE",
      }),
    onSettled: () => qc.invalidateQueries({ queryKey: watchlistKeys.all }),
  })
}
