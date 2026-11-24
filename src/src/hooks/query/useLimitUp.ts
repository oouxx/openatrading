import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { limitUpKeys } from "./queryKeys"
import type { Stock } from "@/stores/data"

export function useLimitUp() {
  return useQuery({
    queryKey: limitUpKeys.all,
    queryFn: () => fetchJson<Stock[]>("/api/limit-up"),
    // 榜单要跟着盘中变化走：服务端缓存 10 秒，前端 15 秒一轮不会额外压上游；
    // 页面不可见时不刷（切回来再补）。
    staleTime: 10 * 1000,
    refetchInterval: 15 * 1000,
    refetchIntervalInBackground: false,
  })
}
