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
    // 快照只负责"有谁、什么顺序"：服务端缓存本身是 5 秒，之前每 3 秒整表重拉既
    // 撞缓存做无用功，又让整张表重渲染。实时跳动交给可视区订阅（`useLiveQuotes`）。
    // 10 秒是"顺序滞后"与"上游请求量"的折中：最坏 ~13s（10 + 5 缓存 + 上游 3）。
    staleTime: 10 * 1000,
    refetchInterval: 10 * 1000,
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
