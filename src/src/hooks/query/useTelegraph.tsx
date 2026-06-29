import { useQuery, useInfiniteQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { telegraphKeys } from "./queryKeys"

export function useTelegraph() {
  return useQuery({
    queryKey: telegraphKeys.all,
    queryFn: () => fetchJson("/api/telegraph"),
    staleTime: 30 * 1000,
  })
}

export function useTelegraphInfinite() {
  return useInfiniteQuery({
    queryKey: [...telegraphKeys.all, "infinite"],
    queryFn: ({ pageParam }) => {
      const params = pageParam ? `?last_time=${pageParam}&count=20` : ""
      return fetchJson(`/api/telegraph${params}`)
    },
    initialPageParam: undefined as number | undefined,
    getNextPageParam: (lastPage: any[]) => {
      if (!lastPage || lastPage.length < 20) return undefined
      // 用最后一页最旧项的 ctime 作为 next cursor
      const last = lastPage[lastPage.length - 1]
      return last?.ctime ?? last?.modified_time ?? undefined
    },
    staleTime: 30 * 1000,
  })
}
