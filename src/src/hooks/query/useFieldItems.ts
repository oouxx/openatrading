import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { fieldItemsKeys } from "./queryKeys"
import type { FieldPlate } from "@/stores/data"

function todayStr(): string {
  return new Date().toISOString().split("T")[0]
}

export function useFieldItems(date?: string) {
  const d = date ?? todayStr()
  return useQuery({
    queryKey: fieldItemsKeys.byDate(d),
    queryFn: () => fetchJson<FieldPlate[]>(`/api/field-items?date=${d}`),
    staleTime: 6 * 60 * 60 * 1000,
  })
}
