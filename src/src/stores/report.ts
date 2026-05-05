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
  risk_level: string
  opportunity_signals: string[]
  decision_summary: string
}

const LS_KEY = 'trading-agent-api-url'

function getApiBaseUrl(): string {
  return localStorage.getItem(LS_KEY) || __API_BASE_URL__
}

export function getApiBaseUrlForDisplay(): string {
  return getApiBaseUrl()
}

export function setApiBaseUrl(url: string) {
  localStorage.setItem(LS_KEY, url)
}

const store = reactive({
  report: null as Report | null,
  loading: false,
  error: '',
  source: __DATA_SOURCE__ as string,
})

async function fetchFromApi(): Promise<Report> {
  const base = getApiBaseUrl()
  const resp = await fetch(`${base}/api/report`)
  if (!resp.ok) throw new Error(`API ${resp.status}: ${resp.statusText}`)
  return resp.json()
}

async function fetchFromJson(): Promise<Report> {
  const resp = await fetch('/report.json')
  if (!resp.ok) throw new Error(`JSON ${resp.status}`)
  return resp.json()
}

async function fetchFromWasm(): Promise<Report> {
  const mod = await import('../wasm/trending_agent_wasm.js')
  await mod.default()
  const isDev = import.meta.env.DEV
  mod.set_xgb_base_url(isDev ? '/api/xgb' : 'https://flash-api.xuangubao.cn/api')
  const json = await mod.run_report()
  return JSON.parse(json)
}

async function fetchReport() {
  store.loading = true
  store.error = ''
  try {
    switch (__DATA_SOURCE__) {
      case 'api':
        store.report = await fetchFromApi()
        break
      case 'json':
        store.report = await fetchFromJson()
        break
      case 'wasm':
        store.report = await fetchFromWasm()
        break
    }
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
