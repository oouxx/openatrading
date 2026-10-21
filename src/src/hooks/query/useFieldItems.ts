import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { fieldItemsKeys } from "./queryKeys"
import type { FieldPlate } from "@/stores/data"

const BEIJING_UTC_OFFSET_MS = 8 * 60 * 60 * 1000

/**
 * 后端按北京时间取当日异动，而 `toISOString()` 是 UTC：
 * 北京时间 00:00–08:00 会落到前一天，导致早盘前一直查到昨天的数据。
 */
function beijingToday(): string {
  return new Date(Date.now() + BEIJING_UTC_OFFSET_MS).toISOString().slice(0, 10)
}

export function useFieldItems(date?: string) {
  const d = date ?? beijingToday()
  return useQuery({
    queryKey: fieldItemsKeys.byDate(d),
    queryFn: () => fetchJson<FieldPlate[]>(`/api/field-items?date=${d}`),
    // 异动是盘中实时数据，不能按天缓存；后端盘中还有 10s 缓存兜底。
    staleTime: 30 * 1000,
    refetchInterval: 60 * 1000,
  })
}
