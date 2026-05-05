import { defineStore } from 'pinia'
import { ref, type Ref } from 'vue'
import { runReport } from '../api/web'

// Types matching Rust serde_json output (snake_case)
interface PlateReason {
  plate_id: string
  plate_name: string
  plate_reason: string | null
}

interface SurgeReason {
  symbol: string
  stock_reason: string
  related_plates: PlateReason[]
}

interface Stock {
  symbol: string
  stock_chi_name: string
  price: number
  change_percent: number
  limit_up_days: number
  break_limit_up_times: number
  turnover_ratio: number
  total_capital: number
  surge_reason: SurgeReason | null
}

interface MarketOverview {
  rise_count: number
  fall_count: number
  limit_up_count: number
  limit_down_count: number
  bomb_rate: number
}

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface PlateAbnormalEventData {
  plate_id: number | null
  plate_name: string | null
  pcp: number | null
  related_stocks: RelatedStock[]
}

interface PlateAbnormalEvent {
  id: number
  target: string
  event_type: number
  event_timestamp: number
  plate_abnormal_event_data: PlateAbnormalEventData | null
  good_or_bad: number
}

interface CommunityUser {
  user_id: string
  nickname: string
  avatar: string | null
}

interface CommunityStock {
  code: string
  name: string
}

interface CommunityArticle {
  article_id: string
  title: string
  create_time: string
  content: string | null
  like_count: number
  comment_count: number
  forward_count: number
  collect_count: number
  user: CommunityUser
  stock_list: CommunityStock[]
}

interface SentimentResult {
  hot_stocks: [string, string, number][]
  hot_articles: [string, number][]
  trend: string
  summary: string
  news_summary: string
  articles: CommunityArticle[]
}

interface RoleAnalysis {
  role: string
  content: string
  elapsed_ms: number
}

interface TgbUserSummary {
  user_id: string
  nickname: string
  post_count: number
  top_post_title: string
  last_update: string
}

interface XuanGubaoExtra {
  limit_down: Stock[]
  plate_abnormal: PlateAbnormalEvent[]
}

interface FieldStock {
  code: string
  name: string
}

interface FieldPlate {
  name: string
  count: number
  reason: string
  list: FieldStock[]
}

interface TimelineDay {
  date: string
  list: { title: string; content: string }[]
}

interface Report {
  date: string
  overview: MarketOverview
  limit_up: Stock[]
  plate_abnormal: PlateAbnormalEvent[]
  field_items: FieldPlate[]
  timeline: TimelineDay[]
  analyses: RoleAnalysis[]
  sentiment: SentimentResult | null
  risk_level: string
  opportunity_signals: string[]
  decision_summary: string
  taoguba_users: TgbUserSummary[]
  xuangubao_extra: XuanGubaoExtra
}

export const useReportStore = defineStore('report', () => {
  const report: Ref<Report | null> = ref(null)
  const loading = ref(false)
  const error: Ref<string | null> = ref(null)

  async function fetchReport() {
    loading.value = true
    error.value = null
    try {
      const jsonStr = await runReport()
      report.value = JSON.parse(jsonStr) as Report
    } catch (e) {
      console.error('Failed to fetch report:', e)
      error.value = String(e)
    } finally {
      loading.value = false
    }
  }

  return { report, loading, error, fetchReport }
})
