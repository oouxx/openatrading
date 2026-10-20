import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { roleKeys } from "./queryKeys"
import type {
  RoleEntry,
  RolePosition,
  RoleAccount,
  RoleTrade,
  RoleLog,
  PnlHistoryResponse,
  ClosedPosition,
} from "@/stores/data"

/**
 * 持仓/账户包含实时行情估值，必须轮询；否则页面停留时盈亏会一直冻结。
 * 30 秒是行情刷新频率与后端 quotes 请求量之间的折中。
 */
export const ROLE_LIVE_REFRESH_MS = 30_000

/** 侧边栏需要同时刷新所有角色，频率降一档，避免 N 个角色放大行情请求。 */
export const ROLE_SIDEBAR_REFRESH_MS = 60_000

export function useRoleList() {
  return useQuery({
    queryKey: roleKeys.list(),
    queryFn: () => fetchJson<RoleEntry[]>("/api/roles"),
    staleTime: 60_000,
    refetchOnWindowFocus: true,
  })
}

export function useRolePositions(name: string) {
  return useQuery({
    queryKey: roleKeys.positions(name),
    queryFn: () => fetchJson<RolePosition[]>(`/api/roles/${name}/positions`),
    staleTime: 10_000,
    refetchInterval: ROLE_LIVE_REFRESH_MS,
    refetchOnWindowFocus: true,
    enabled: !!name,
  })
}

export function useRoleAccount(name: string) {
  return useQuery({
    queryKey: roleKeys.account(name),
    queryFn: () => fetchJson<RoleAccount>(`/api/roles/${name}/account`),
    staleTime: 10_000,
    refetchInterval: ROLE_LIVE_REFRESH_MS,
    refetchOnWindowFocus: true,
    enabled: !!name,
  })
}

export function useRoleTrades(name: string, limit = 100) {
  return useQuery({
    queryKey: roleKeys.trades(name, limit),
    queryFn: () => fetchJson<RoleTrade[]>(`/api/roles/${name}/trades?limit=${limit}`),
    staleTime: 30_000,
    refetchOnWindowFocus: true,
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
    staleTime: 30_000,
    // 曲线末点用实时行情估值，和持仓/账户保持同一量级的刷新频率。
    refetchInterval: ROLE_SIDEBAR_REFRESH_MS,
    refetchOnWindowFocus: true,
    enabled: !!name,
  })
}

export function useRoleHistory(name: string, limit = 100) {
  return useQuery({
    queryKey: roleKeys.history(name, limit),
    queryFn: () => fetchJson<ClosedPosition[]>(`/api/roles/${name}/history?limit=${limit}`),
    staleTime: 60_000,
    refetchOnWindowFocus: true,
    enabled: !!name,
  })
}
