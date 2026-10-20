import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { dapanAbnormalKeys } from "./queryKeys"

/** 大盘异动（/api/dapan-abnormal）。 */
export interface DapanAbnormal {
  id: string
  title: string
  bk_yi_dong_type: number
  created_at: number
  bkj_info?: { id?: string; name?: string }
}

export function useDapanAbnormal() {
  return useQuery({
    queryKey: dapanAbnormalKeys.all,
    queryFn: () => fetchJson<DapanAbnormal[]>("/api/dapan-abnormal"),
    staleTime: 30 * 1000,
  })
}
