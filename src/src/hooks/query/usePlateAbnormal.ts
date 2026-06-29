import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { plateAbnormalKeys } from "./queryKeys"

export function usePlateAbnormal() {
  return useQuery({
    queryKey: plateAbnormalKeys.all,
    queryFn: () => fetchJson<unknown[]>("/api/plate-abnormal"),
    staleTime: 30 * 1000,
  })
}
