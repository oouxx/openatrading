import { useInfiniteQuery } from "@tanstack/react-query"

import { fetchJson } from "@/stores/data"
import { stockRankingKeys } from "./queryKeys"

export type StockRankingOrder = "gainers" | "losers"
export type StockMarketFilter = "all" | "main" | "chinext" | "star" | "beijing"

export interface RankedStock {
  code: string
  symbol: string
  name: string
  price: number | null
  change_percent: number | null
  turnover_rate: number | null
  total_market_cap: number | null
  float_market_cap: number | null
}

export interface StockRankingPage {
  items: RankedStock[]
  next_offset: number | null
}

export function useStockRanking(
  order: StockRankingOrder,
  market: StockMarketFilter,
  excludeSt: boolean,
  limit = 100
) {
  return useInfiniteQuery({
    queryKey: stockRankingKeys.list(order, market, excludeSt, limit),
    queryFn: ({ pageParam }) =>
      fetchJson<StockRankingPage>(
        `/api/stock-rankings?order=${order}&market=${market}&excludeSt=${excludeSt ? "true" : "false"}&offset=${pageParam}&limit=${limit}`
      ),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.next_offset ?? undefined,
    staleTime: 15 * 1000,
    refetchInterval: 30 * 1000,
  })
}
