import { useQuery, useInfiniteQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { telegraphKeys } from "./queryKeys"

/** 财联社电报条目（/api/telegraph）。 */
export interface TelegraphItem {
  id?: number
  level?: string
  ctime?: number | string
  modified_time?: number | string
  brief?: string
  title?: string
  content?: string
}

export function useTelegraph() {
  return useQuery({
    queryKey: telegraphKeys.all,
    queryFn: () => fetchJson<TelegraphItem[]>("/api/telegraph"),
    staleTime: 30 * 1000,
  })
}

export function useTelegraphInfinite() {
  return useInfiniteQuery({
    queryKey: [...telegraphKeys.all, "infinite"],
    queryFn: ({ pageParam }) => {
      const params = pageParam ? `?last_time=${pageParam}&count=20` : ""
      return fetchJson<TelegraphItem[]>(`/api/telegraph${params}`)
    },
    initialPageParam: undefined as string | number | undefined,
    getNextPageParam: (lastPage: TelegraphItem[]) => {
      if (!lastPage || lastPage.length < 20) return undefined
      // 用最后一页最旧项的 ctime 作为 next cursor
      const last = lastPage[lastPage.length - 1]
      return last?.ctime ?? last?.modified_time ?? undefined
    },
    staleTime: 30 * 1000,
  })
}
