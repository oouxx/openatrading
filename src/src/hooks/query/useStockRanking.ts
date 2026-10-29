import { useInfiniteQuery } from "@tanstack/react-query"

import { fetchJson } from "@/stores/data"
import { stockRankingKeys } from "./queryKeys"

export type StockRankingOrder = "gainers" | "losers"
export type StockMarketFilter = "all" | "main" | "chinext" | "star"

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
  generated_at: number
  stale: boolean
}

interface StockRankingPageParam {
  offset: number
  limit: number
}

const INITIAL_PAGE_SIZE = 30
const NEXT_PAGE_SIZE = 100

export function stockRankingQueryOptions(
  order: StockRankingOrder,
  market: StockMarketFilter,
  excludeSt: boolean
) {
  return {
    queryKey: stockRankingKeys.list(order, market, excludeSt),
    queryFn: ({ pageParam }: { pageParam: StockRankingPageParam }) =>
      fetchJson<StockRankingPage>(
        `/api/stock-rankings?order=${order}&market=${market}&excludeSt=${excludeSt ? "true" : "false"}&offset=${pageParam.offset}&limit=${pageParam.limit}`
      ),
    initialPageParam: { offset: 0, limit: INITIAL_PAGE_SIZE } as StockRankingPageParam,
    getNextPageParam: (lastPage: StockRankingPage): StockRankingPageParam | undefined =>
      lastPage.next_offset === null
        ? undefined
        : { offset: lastPage.next_offset, limit: NEXT_PAGE_SIZE },
    staleTime: 2 * 1000,
    refetchInterval: 3 * 1000,
    refetchIntervalInBackground: false,
  }
}

export function useStockRanking(
  order: StockRankingOrder,
  market: StockMarketFilter,
  excludeSt: boolean
) {
  return useInfiniteQuery(stockRankingQueryOptions(order, market, excludeSt))
}
