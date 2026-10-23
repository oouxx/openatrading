import { keepPreviousData, useQuery } from "@tanstack/react-query"
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
  /** 上榜交易日，单日接口下只有一个 */
  trade_dates?: string[]
  /** 去重后的上榜原因 */
  reasons?: string[]
  /** 当日参与该股的营业部 */
  dept_names?: string[]
}

export interface DragonTigerDetailResponse {
  /** 后端实际返回数据的交易日（不传 date 时为最近交易日） */
  date: string
  data: DragonTigerDetailItem[]
  /** 当日出现过的全部营业部，按个股覆盖数倒序 */
  depts: string[]
}

export function useDragonTigerDetail(date?: string) {
  const query = date ? `?date=${encodeURIComponent(date)}` : ""
  return useQuery({
    queryKey: dragonTigerKeys.detail(date ?? ""),
    queryFn: () => fetchJson<DragonTigerDetailResponse>(`/api/dragon-tiger/detail${query}`),
    placeholderData: keepPreviousData,
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
