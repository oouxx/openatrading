import { reactive } from 'vue'

interface Report {
  date: string
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
  }
  limit_up: Array<{
    symbol: string
    stock_chi_name: string
    price: number
    change_percent: number
    limit_up_days: number
    turnover_ratio: number
    total_capital: number
    break_limit_up_times: number
    surge_reason: {
      stock_reason: string
      related_plates: Array<{ plate_name: string }>
    } | null
  }>
  plate_abnormal: Array<any>
  field_items: Array<{
    name: string
    count: number
    reason: string
    list: Array<{
      code: string
      name: string
      article?: {
        action_info?: {
          time?: string
          num?: string
          expound?: string
        }
      }
    }>
  }>
  timeline: Array<{ date: string; list: any[] }>
  sentiment?: {
    trend: string
    hot_stocks: [string, string, number][]
    hot_articles: Array<{
      article_id: string
      title: string
      like_count: number
      comment_count: number
      forward_count: number
      heat_score: number
      nickname: string
      content: string
    }>
    summary: string
    taoguba_articles: Array<{
      topic_id: number
      new_topic_id: string
      subject: string
      subinfo: string
      user_name: string
      view_count: number
      reply_count: number
      date_time: string
      content: string
    }>
  }
  risk_level: string
  opportunity_signals: string[]
  decision_summary: string
}

function getApiBaseUrl(): string {
  return localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
}

const store = reactive({
  report: null as Report | null,
  loading: false,
  error: '',
})

async function fetchReport() {
  store.loading = true
  store.error = ''
  try {
    const base = getApiBaseUrl()
    const resp = await fetch(`${base}/api/report`)
    if (!resp.ok) throw new Error(`API ${resp.status}: ${resp.statusText}`)
    store.report = await resp.json()
  } catch (e: any) {
    store.error = e.message || String(e)
    console.error('Fetch failed:', e)
  } finally {
    store.loading = false
  }
}

export function useReportStore() {
  return store
}

export { fetchReport }
