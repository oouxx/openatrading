import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { yesterdayLimitUpKeys } from "./queryKeys"
import type { Stock } from "@/stores/data"

export function useYesterdayLimitUp() {
  return useQuery({
    queryKey: yesterdayLimitUpKeys.all,
    queryFn: () => fetchJson<Stock[]>("/api/yesterday-limit-up"),
    // 昨日涨停的**名单**当天开盘后就固定了，只有"今天表现"会在意时间差；而这个接口
    // 单次 250KB（150 行 × 全字段），15 秒一轮太浪费。给 60 秒：晋级分析里真正在动的
    // 今日涨停名单由 `useLimitUp` 15 秒刷。
    staleTime: 30 * 1000,
    refetchInterval: 60 * 1000,
    refetchIntervalInBackground: false,
  })
}
