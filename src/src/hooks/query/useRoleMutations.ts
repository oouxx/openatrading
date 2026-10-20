import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { OrderSide, PlaceOrderResponse, RoleEntry } from "@/stores/data"
import { roleKeys } from "./queryKeys"

/**
 * 一笔成交会同时改变持仓、账户、成交流水、平仓历史与盈利曲线，
 * 统一在这里失效，避免每个 mutation 各写一遍。
 */
function invalidateRoleTradingData(queryClient: QueryClient, name: string) {
  queryClient.invalidateQueries({ queryKey: roleKeys.positions(name) })
  queryClient.invalidateQueries({ queryKey: roleKeys.account(name) })
  queryClient.invalidateQueries({ queryKey: roleKeys.tradesScope(name) })
  queryClient.invalidateQueries({ queryKey: roleKeys.historyScope(name) })
  queryClient.invalidateQueries({ queryKey: roleKeys.pnlHistoryScope(name) })
}

export interface PlaceOrderVars {
  roleName: string
  side: OrderSide
  symbol: string
  /** 后端可省略；省略时不发送该字段 */
  stockName?: string
  shares: number
  price: number
  reason?: string
}

/**
 * 手动下单（模拟盘）。不做撮合，价格由用户填写。
 * 校验失败时后端返回 400 `{ "error": "…" }`，由 fetchJson 抛出为 Error.message。
 */
export function usePlaceOrder() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ roleName, side, symbol, stockName, shares, price, reason }: PlaceOrderVars) =>
      fetchJson<PlaceOrderResponse>(`/api/roles/${roleName}/orders`, {
        method: "POST",
        body: JSON.stringify({
          side,
          symbol,
          stock_name: stockName,
          shares,
          price,
          reason,
        }),
      }),
    onSuccess: (_data, vars) => {
      invalidateRoleTradingData(queryClient, vars.roleName)
    },
  })
}

export interface CreateRoleVars {
  name: string
  displayName: string
  initialCapital: number
}

export function useCreateRole() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ name, displayName, initialCapital }: CreateRoleVars) =>
      fetchJson("/api/roles", {
        method: "POST",
        body: JSON.stringify({
          name,
          display_name: displayName,
          initial_capital: initialCapital,
        }),
      }),
    onSuccess: (_data, vars) => {
      // 先把新角色写进列表缓存：创建成功后立即选中它时，activeRole 的派生
      // 逻辑需要列表里已经有这个角色，否则会回退到旧角色。
      queryClient.setQueryData<RoleEntry[]>(roleKeys.list(), (old) => {
        if (!old || old.some((r) => r.name === vars.name)) return old
        return [
          ...old,
          {
            name: vars.name,
            display_name: vars.displayName,
            readonly: false,
            initial_capital: vars.initialCapital,
          },
        ]
      })
      // 只刷新角色列表；新建角色不会影响已有角色的持仓/账户查询。
      queryClient.invalidateQueries({ queryKey: roleKeys.list() })
    },
  })
}

export function useDeleteRole() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) => fetchJson(`/api/roles/${name}`, { method: "DELETE" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: roleKeys.all })
    },
  })
}

export function useResetRole() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) => fetchJson(`/api/roles/${name}/reset`, { method: "POST" }),
    onSuccess: (_data, name) => {
      invalidateRoleTradingData(queryClient, name)
      queryClient.invalidateQueries({ queryKey: roleKeys.logs(name) })
    },
  })
}
