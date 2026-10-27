import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { plateRankingKeys } from "./queryKeys"

export interface PlateLeaderStock {
  symbol: string
  stock_chi_name: string
  change_percent?: number | null
  price_change?: number | null
}

export interface PlateLeaderStocks {
  items: PlateLeaderStock[]
}

export interface PlateRankingItem {
  plate_id: number
  plate_name: string
  pcp: number
  fund_flow?: number | null
  rise_count: number
  fall_count: number
  stay_count: number
  limit_up_count: number
  core_avg_pcp_rank?: number | null
  core_avg_pcp_rank_change?: number | null
  top_n_stocks: PlateLeaderStocks
  bottom_n_stocks: PlateLeaderStocks
}

export interface PlateRankingsResponse {
  generated_at: number
  gainers: PlateRankingItem[]
  losers: PlateRankingItem[]
}

/** 0=全部，1=概念，2=行业，3=风格。 */
export type PlateRankType = 0 | 1 | 2 | 3

export interface PlateStock {
  symbol: string
  name?: string
  last_px?: number
  px_change?: number
  px_change_rate?: number
  preclose_px?: number
  open_px?: number
  high_px?: number
  low_px?: number
  amplitude?: number
  turnover_ratio?: number
  limit_up_days?: number
  last_limit_up?: number
  limit_status?: number
  nearly_new_acc_pcp?: number
  turnover_volume?: number
  turnover_value?: number
  market_value?: number
  circulation_value?: number
  pe_rate?: number
  dyn_pe?: number
  dyn_pb_rate?: number
  volume_ratio?: number
  circulation_shares?: number
  total_shares?: number
  trade_status?: string
  update_time?: number
  core_flag?: number | null
  desc: string
  desc_url?: string | null
  industrial_chain_id?: number | null
  industrial_chain_name?: string
  is_new: boolean
  need_pay: boolean
  labels?: StockLabel[]
}

export interface StockLabel {
  code: string
  symbol: string
  label_name: string
  label_type: string
  description: string
  effect: number
}

export interface PlateIndustrialChain {
  id: number
  name: string
  order: number
  stocks: string[]
  stock_premium_infos: PlateStock[]
}

export interface PlateQuestionAnswer {
  question: string
  html_answer: string
}

export interface PlateSubset {
  plate_id: number
  plate_name: string
}

export interface PlateDetail {
  id: number
  name: string
  desc: string
  avg_pcp?: number | null
  core_avg_pcp?: number | null
  fund_flow?: number | null
  rise_count: number
  fall_count: number
  stay_count: number
  core_stocks_count: number
  stocks: PlateStock[]
  industrial_chains: PlateIndustrialChain[]
  allow_comment: boolean
  create_name: string
  display_after_at: number
  hang_ye_long_tou_stocks_count: number
  has_north: number
  hide: boolean
  is_fav: boolean
  need_pay: boolean
  premium_desc: string
  question_answers: PlateQuestionAnswer[]
  seo_desc: string
  seo_keywords: string
  seo_title: string
  sub_sets?: PlateSubset[] | null
  subject_id: number
  update_name: string
  whether_display_plate_stock_portfolio: boolean
}

export function usePlateRankings(rankType: PlateRankType) {
  return useQuery({
    queryKey: plateRankingKeys.rankings(rankType),
    queryFn: () => fetchJson<PlateRankingsResponse>(`/api/plate-rankings?type=${rankType}`),
    staleTime: 30 * 1000,
    refetchInterval: 30 * 1000,
  })
}

export function usePlateDetail(plateId: number) {
  return useQuery({
    queryKey: plateRankingKeys.detail(plateId),
    queryFn: () => fetchJson<PlateDetail>(`/api/plate-details/${plateId}`),
    staleTime: 60 * 1000,
    enabled: plateId > 0,
  })
}
