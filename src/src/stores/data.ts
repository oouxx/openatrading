import { reactive } from 'vue'

// ── Helpers ────────────────────────────────────────────

function getApiBaseUrl(): string {
  return localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
}

async function fetchJson<T>(path: string): Promise<T> {
  const base = getApiBaseUrl()
  const resp = await fetch(`${base}${path}`)
  if (!resp.ok) throw new Error(`${resp.status}`)
  return resp.json()
}

function todayStr(): string {
  return new Date().toISOString().split('T')[0]
}

// ── Types ──────────────────────────────────────────────

export interface OverviewData {
  date: string
  rise_count: number
  fall_count: number
  limit_up_count: number
  limit_down_count: number
  bomb_rate: number
  risk_level: string
  decision_summary: string
  // Optional extended fields from /api/overview
  sentiment_temp?: number
  promotion_rate?: number
  promotion_history_avg?: number
  net_flow?: number
  flow_sector?: string
}

export interface Stock {
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
}

export interface FieldPlate {
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
}

// ── Overview ───────────────────────────────────────────

export const overview = reactive({
  data: null as OverviewData | null,
  loading: false,
  error: '',
})

export async function fetchOverview() {
  overview.loading = true
  overview.error = ''
  try {
    overview.data = await fetchJson<OverviewData>('/api/overview')
  } catch (e: any) {
    overview.error = e.message || String(e)
  } finally {
    overview.loading = false
  }
}

// ── Limit Up ───────────────────────────────────────────

export const limitUp = reactive({
  data: [] as Stock[],
  loading: false,
  error: '',
})

export async function fetchLimitUp() {
  limitUp.loading = true
  limitUp.error = ''
  try {
    limitUp.data = await fetchJson<Stock[]>('/api/limit-up')
  } catch (e: any) {
    limitUp.error = e.message || String(e)
  } finally {
    limitUp.loading = false
  }
}

// ── Field Items ────────────────────────────────────────

export const fieldItems = reactive({
  data: [] as FieldPlate[],
  loading: false,
  error: '',
  date: todayStr(),
})

export async function fetchFieldItems(date?: string) {
  fieldItems.loading = true
  fieldItems.error = ''
  const d = date || fieldItems.date
  try {
    fieldItems.data = await fetchJson<FieldPlate[]>(`/api/field-items?date=${d}`)
    fieldItems.date = d
  } catch (e: any) {
    fieldItems.error = e.message || String(e)
  } finally {
    fieldItems.loading = false
  }
}

// ── Plate Abnormal ─────────────────────────────────────

export const plateAbnormal = reactive({
  data: [] as any[],
  loading: false,
  error: '',
})

export async function fetchPlateAbnormal() {
  plateAbnormal.loading = true
  plateAbnormal.error = ''
  try {
    plateAbnormal.data = await fetchJson<any[]>('/api/plate-abnormal')
  } catch (e: any) {
    plateAbnormal.error = e.message || String(e)
  } finally {
    plateAbnormal.loading = false
  }
}

// ── Timeline ───────────────────────────────────────────

export const timeline = reactive({
  data: [] as any[],
  loading: false,
  error: '',
})

export async function fetchTimeline() {
  timeline.loading = true
  timeline.error = ''
  try {
    timeline.data = await fetchJson<any[]>('/api/timeline')
  } catch (e: any) {
    timeline.error = e.message || String(e)
  } finally {
    timeline.loading = false
  }
}

// ── Super Stock ────────────────────────────────────────

export const superStock = reactive({
  data: [] as Stock[],
  loading: false,
  error: '',
})

export async function fetchSuperStock() {
  superStock.loading = true
  superStock.error = ''
  try {
    superStock.data = await fetchJson<Stock[]>('/api/super-stock')
  } catch (e: any) {
    superStock.error = e.message || String(e)
  } finally {
    superStock.loading = false
  }
}

// ── Sentiment ──────────────────────────────────────────

export const sentiment = reactive({
  data: null as any,
  loading: false,
  error: '',
})

export async function fetchSentiment() {
  sentiment.loading = true
  sentiment.error = ''
  try {
    sentiment.data = await fetchJson<any>('/api/sentiment')
  } catch (e: any) {
    sentiment.error = e.message || String(e)
  } finally {
    sentiment.loading = false
  }
}
