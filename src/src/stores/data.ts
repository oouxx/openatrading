import { useSettingsStore } from "@/stores/settingsStore"

const isTauri = "__TAURI_INTERNALS__" in window

/**
 * A stored apiUrl pointing at localhost/127.0.0.1 is only valid when the page
 * itself is served from localhost (local dev). On a deployed site it is a stale
 * value left over from local dev and must NOT override the baked/default base —
 * otherwise the browser tries ws://localhost:8080 and http://localhost:8080
 * and every API call (REST, SSE, WebSocket) fails. Drop it in that case so we
 * fall back to the same-origin / __API_BASE_URL__ default.
 */
function resolveStoredApiUrl(): string {
  const stored = useSettingsStore.getState().apiUrl
  if (!stored) return ""
  const localhostRe = /^https?:\/\/(localhost|127\.0\.0\.1)([:/]|$)/i
  const storedIsLocal = localhostRe.test(stored)
  const pageIsLocal = typeof window !== "undefined" && localhostRe.test(window.location.origin)
  return storedIsLocal && !pageIsLocal ? "" : stored
}

export function getApiBaseUrl(): string {
  if (isTauri) return ""
  return resolveStoredApiUrl() || __API_BASE_URL__
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    if (init?.method === "POST" || init?.method === "PUT" || init?.method === "DELETE") {
      let body: Record<string, unknown> = {}
      if (init.body) {
        try {
          body = JSON.parse(String(init.body))
        } catch {
          /* use empty */
        }
      }
      return invoke("api_post", { path, body }) as Promise<T>
    }
    if (init?.method === "PATCH") {
      let body: Record<string, unknown> = {}
      if (init.body) {
        try {
          body = JSON.parse(String(init.body))
        } catch {
          /* use empty */
        }
      }
      return invoke("api_patch", { path, body }) as Promise<T>
    }
    return invoke("api_fetch", { path }) as Promise<T>
  }
  const base = getApiBaseUrl()
  const headers: Record<string, string> = {}
  if (init?.body) headers["Content-Type"] = "application/json"
  const resp = await fetch(`${base}${path}`, {
    ...init,
    headers: { ...headers, ...(init?.headers as Record<string, string>) },
  })
  if (!resp.ok) throw new Error(`${resp.status}`)
  if (resp.status === 204) return undefined as T
  return resp.json()
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
  market_temperature: number
  yesterday_limit_up_avg_pcp: number
}

export interface SentimentHistoryDay {
  date: string
  sentiment: number
  limit_up_count: number
  bomb_rate: number
  rise_ratio: number
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
  yesterday_limit_up_days?: number
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
        shares_range?: number
        day?: number
        edition?: number
        price?: number
      }
    }
  }>
}

export interface RoleEntry {
  name: string
  display_name: string
  readonly?: boolean
}

export interface RolePosition {
  id: number
  symbol: string
  stock_name: string
  cost_price: number
  current_price: number
  shares: number
  entry_reason: string
  entry_date: string
}

export interface RoleLog {
  date: string
  action: string
  symbol: string
  price: number
  shares: number
  reason: string
}

export interface PnlPoint {
  date: string
  equity: number
  cash: number
  market_value: number
  return_pct: number
}

export interface PnlHistoryResponse {
  points: PnlPoint[]
  initial_capital: number
}

export interface ClosedPosition {
  position_id: number
  symbol: string
  stock_name: string
  entry_date: string
  exit_date: string
  entry_price: number
  exit_price: number
  shares: number
  pnl_pct: number
  entry_reason: string
  exit_reason: string
}

export interface IndexQuote {
  symbol: string
  name: string
  last_px: number
  px_change: number
  px_change_rate: number
  trade_status: string
}
