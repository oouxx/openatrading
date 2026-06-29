import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { dragonTigerKeys } from "./queryKeys"

export interface SeatDetail {
  dept_name: string
  seat_type: string | null
  buy_amt: number | null
  buy_ratio: number | null
  sell_amt: number | null
  sell_ratio: number | null
  net_amt: number | null
}

export interface DragonTigerDetailItem {
  code: string
  name: string
  trade_date: string
  close_price: number | null
  change_rate: number | null
  billboard_net_amt: number | null
  billboard_buy_amt: number | null
  billboard_sell_amt: number | null
  billboard_deal_amt: number | null
  accum_amount: number | null
  turnover_rate: number | null
  free_market_cap: number | null
  explanation: string | null
  d1_chg: number | null
  d2_chg: number | null
  d5_chg: number | null
  d10_chg: number | null
}

export interface DragonTigerDetailResponse {
  data: DragonTigerDetailItem[]
  total: number
  page: number
  page_size: number
  total_net_amt: number
  net_buy_count: number
  net_sell_count: number
}

export function useDragonTigerDetail(page: number, pageSize: number) {
  return useQuery({
    queryKey: dragonTigerKeys.detail(page, pageSize),
    queryFn: () =>
      fetchJson<DragonTigerDetailResponse>(
        `/api/dragon-tiger/detail?page=${page}&page_size=${pageSize}`
      ),
    staleTime: 60 * 1000,
  })
}

export function useDragonTigerStockSeat(
  code: string | null,
  date: string | null,
  flag: "buy" | "sell"
) {
  return useQuery({
    queryKey: dragonTigerKeys.stockSeat(code ?? "", date ?? "", flag),
    queryFn: () =>
      fetchJson<SeatDetail[]>(
        `/api/dragon-tiger/stock-seat?code=${code}&date=${date}&flag=${flag}`
      ),
    enabled: code != null && date != null,
    staleTime: 60 * 1000,
  })
}
