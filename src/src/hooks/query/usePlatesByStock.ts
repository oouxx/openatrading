import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { platesByStockKeys } from "./queryKeys"

/** 个股所属板块（/api/plates-by-any-stock/:symbol）。 */
export interface PlateByStockItem {
  plate_id: number
  plate_name: string
  core_avg_pcp: number
  reason?: string
}

/**
 * 个股所属板块。`core_avg_pcp` 是**板块涨幅**，会跟着盘面走，所以这个查询是
 * 轮询的——挂成「所属板块」格子后它可能在屏幕上一待一整天，只取一次快照的话
 * 那个百分比就永远停在打开那一刻。
 *
 * 3 秒一轮，和盘口同一档：后端用的是 `CachePolicy::tick()`（交易时段 3 秒过期，
 * 休市缓存到下次开盘），所以每一轮都拿得到新数据，休市时的请求也只是纯内存读、
 * 不碰上游。页面切到后台时 react-query 默认暂停轮询，不会在后台空转。
 */
export function usePlatesByStock(symbol: string) {
  return useQuery({
    queryKey: platesByStockKeys.bySymbol(symbol),
    queryFn: () => fetchJson<PlateByStockItem[]>(`/api/plates-by-any-stock/${symbol}`),
    staleTime: 3 * 1000,
    refetchInterval: 3 * 1000,
    enabled: !!symbol,
  })
}
