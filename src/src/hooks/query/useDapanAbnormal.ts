import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { dapanAbnormalKeys } from "./queryKeys"

export function useDapanAbnormal() {
  return useQuery({
    queryKey: dapanAbnormalKeys.all,
    queryFn: () => fetchJson<unknown[]>("/api/dapan-abnormal"),
    staleTime: 30 * 1000,
  })
}
