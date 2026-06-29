import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { roleKeys } from "./queryKeys"
import type {
  RoleEntry,
  RolePosition,
  RoleLog,
  PnlHistoryResponse,
  ClosedPosition,
} from "@/stores/data"

export function useRoleList() {
  return useQuery({
    queryKey: roleKeys.list(),
    queryFn: () => fetchJson<RoleEntry[]>("/api/roles"),
    staleTime: 5 * 60 * 1000,
  })
}

export function useRolePositions(name: string) {
  return useQuery({
    queryKey: roleKeys.positions(name),
    queryFn: () => fetchJson<RolePosition[]>(`/api/roles/${name}/positions`),
    staleTime: 5 * 60 * 1000,
    enabled: !!name,
  })
}

export function useRoleLogs(name: string) {
  return useQuery({
    queryKey: roleKeys.logs(name),
    queryFn: () => fetchJson<RoleLog[]>(`/api/roles/${name}/logs`),
    staleTime: 5 * 60 * 1000,
    enabled: !!name,
  })
}

export function useRolePnlHistory(name: string, days = 30) {
  return useQuery({
    queryKey: roleKeys.pnlHistory(name, days),
    queryFn: async () => {
      const res = await fetchJson<PnlHistoryResponse>(`/api/roles/${name}/pnl-history?days=${days}`)
      return res.points
    },
    staleTime: 5 * 60 * 1000,
    enabled: !!name,
  })
}

export function useRoleHistory(name: string, limit = 100) {
  return useQuery({
    queryKey: roleKeys.history(name, limit),
    queryFn: () => fetchJson<ClosedPosition[]>(`/api/roles/${name}/history?limit=${limit}`),
    staleTime: 5 * 60 * 1000,
    enabled: !!name,
  })
}
