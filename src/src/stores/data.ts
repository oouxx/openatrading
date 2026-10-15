import { useSettingsStore } from "@/stores/settingsStore"

import { isTauri } from "@/lib/env"
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

/**
 * 把后端错误响应转成用户可读的错误信息。
 *
 * 后端校验失败会返回 `{ "error": "股数必须是 100 的整数倍" }`，这类业务原因
 * 必须原样透出给用户；body 不是 JSON、或没有 `error` 字段时回退到兜底文案
 * （web 端是 HTTP 状态码，Tauri 端是 invoke 的原始错误串）。
 */
function toErrorMessage(raw: unknown, fallback: string): string {
  const text = typeof raw === "string" ? raw.trim() : ""
  if (!text) return fallback
  if (text.startsWith("{")) {
    try {
      const parsed: unknown = JSON.parse(text)
      const err = (parsed as { error?: unknown }).error
      if (typeof err === "string" && err.trim()) return err.trim()
    } catch {
      /* 非法 JSON —— 按纯文本处理 */
    }
    return fallback
  }
  return text
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const parseBody = (): Record<string, unknown> => {
    if (!init?.body) return {}
    try {
      return JSON.parse(String(init.body)) as Record<string, unknown>
    } catch {
      return {}
    }
  }
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    try {
      if (init?.method === "POST" || init?.method === "PUT" || init?.method === "DELETE") {
        return (await invoke("api_post", { path, body: parseBody() })) as T
      }
      if (init?.method === "PATCH") {
        return (await invoke("api_patch", { path, body: parseBody() })) as T
      }
      return (await invoke("api_fetch", { path })) as T
    } catch (e) {
      // Tauri 端把后端错误直接作为字符串 reject（可能带 `{ "error": ... }` 结构）
      throw new Error(toErrorMessage(e, e instanceof Error ? e.message : "请求失败"), { cause: e })
    }
  }
  const base = getApiBaseUrl()
  const headers: Record<string, string> = {}
  if (init?.body) headers["Content-Type"] = "application/json"
  const resp = await fetch(`${base}${path}`, {
    ...init,
    headers: { ...headers, ...(init?.headers as Record<string, string>) },
  })
  if (!resp.ok) {
    const body = await resp.text().catch(() => "")
    throw new Error(toErrorMessage(body, `${resp.status}`))
  }
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
  /** 该角色的初始资金（元）。建角色时设定，之后不可修改——要重新开始只能重置账户 */
  initial_capital: number
}

export interface RolePosition {
  role_name: string
  symbol: string
  stock_name: string
  shares: number
  cost_price: number
  /** 建仓日期，格式 YYYY-MM-DD */
  opened_at: string
  /** 以下估值字段在行情不可用时为 null，UI 必须显示 “—” 而不是 0 */
  last_price: number | null
  market_value: number | null
  unrealized_pnl: number | null
  /** 浮盈百分比，单位是百分数（2.9 表示 2.9%） */
  unrealized_pct: number | null
}

/** 账户概览 —— GET /api/roles/:name/account */
export interface RoleAccount {
  role_name: string
  initial_capital: number
  cash: number
  market_value: number
  equity: number
  unrealized_pnl: number
  realized_pnl: number
  total_pnl: number
  /** 总盈亏百分比，单位是百分数（0.1 表示 0.1%） */
  total_pnl_pct: number
  /** false 表示部分持仓行情拉取失败，市值/权益是不完整值 */
  valuation_complete: boolean
}

export type OrderSide = "buy" | "sell"

/** 下单请求体 —— POST /api/roles/:name/orders */
export interface PlaceOrderPayload {
  side: OrderSide
  symbol: string
  stock_name?: string
  shares: number
  price: number
  reason?: string
}

/** 成交流水 —— GET /api/roles/:name/trades */
export interface RoleTrade {
  id: number
  role_name: string
  symbol: string
  stock_name: string
  side: OrderSide
  price: number
  shares: number
  cost_price: number
  /** 买入为 null */
  realized_pnl: number | null
  reason: string
  traded_at: string
}

/** 下单成功响应 —— 全部卖光时 position 为 null */
export interface PlaceOrderResponse {
  trade: RoleTrade
  position: RolePosition | null
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

/** 已平仓记录 —— GET /api/roles/:name/history（只包含卖出成交） */
export interface ClosedPosition {
  id: number
  role_name: string
  symbol: string
  stock_name: string
  shares: number
  cost_price: number
  exit_price: number
  realized_pnl: number
  /** 已实现盈亏百分比，单位是百分数（-0.2 表示 -0.2%） */
  realized_pct: number
  closed_at: string
  reason: string
}

/** 股票搜索建议项 —— GET /api/search-stocks?q= */
export interface StockSearchItem {
  code: string
  name: string
}

export interface IndexQuote {
  symbol: string
  name: string
  last_px: number
  px_change: number
  px_change_rate: number
  trade_status: string
}
