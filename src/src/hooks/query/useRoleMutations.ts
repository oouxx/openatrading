import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { OrderSide, PlaceOrderResponse } from "@/stores/data"
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

export function useCreateRole() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({
      name,
      displayName,
    }: {
      name: string
      displayName: string
    }) =>
      fetchJson("/api/roles", {
        method: "POST",
        body: JSON.stringify({ name, display_name: displayName }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: roleKeys.all })
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
