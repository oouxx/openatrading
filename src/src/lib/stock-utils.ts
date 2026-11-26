// Shared stock symbol and chart URL utilities

import { fetchJson } from "@/stores/data"
import type { StockSearchItem } from "@/stores/data"

// ── Stock search ───────────────────────────────────────

/**
 * 按名称或代码搜索 A 股（后端 `/api/search-stocks`，数据源为东方财富 suggest 接口）。
 * web 与 Tauri 两个运行时的差异由 `fetchJson` 统一抹平，调用方不需要分支。
 */
export async function searchStocks(keyword: string): Promise<StockSearchItem[]> {
  const q = keyword.trim()
  if (!q) return []
  return fetchJson<StockSearchItem[]>(`/api/search-stocks?q=${encodeURIComponent(q)}`)
}

// ── Symbol normalization (mirrors backend crates/api/src/utils.rs to_yahoo_symbol) ──

const YAHOO_SUFFIXES = ["SS", "SZ", "BJ", "HK", "TW"]

function parsePrefixed(input: string): [string | null, string] {
  const upper = input.toUpperCase()
  for (const prefix of ["SH", "SZ", "SS", "BJ"]) {
    if (upper.startsWith(prefix)) {
      const rest = input.slice(prefix.length).replace(/^[.\-_]+/, "")
      return [input.slice(0, prefix.length), rest]
    }
  }
  return [null, input]
}

function normalizeCode(raw: string): string {
  const digits = raw.replace(/\D/g, "")
  if (!digits) throw new Error(`输入不含任何数字: ${raw}`)
  if (digits.length > 6) throw new Error(`代码位数超过6位: ${digits}`)
  return digits.padStart(6, "0")
}

function inferExchange(code: string): "SS" | "SZ" | "BJ" {
  if (code.length < 6 || !/^\d{6}$/.test(code)) {
    throw new Error(`代码格式不正确，需为 6 位数字代码: '${code}'`)
  }

  const p2 = code.slice(0, 2)
  const p3 = code.slice(0, 3)

  // 北交所：43/83/87/88 开头的存量代码，920 开头为 2023 年后新股代码段，
  // 899 为北证指数（北证 50）。
  if (["43", "83", "87", "88"].includes(p2) || p3 === "920" || p3 === "899") return "BJ"

  // 深证指数：399xxx（深市个股、ETF、债券都不占这一段）
  if (p3 === "399") return "SZ"

  // 上海证券交易所：个股 + 全部场内基金（5xxxxx 是沪市基金专用段，
  // ETF/LOF/封闭式基金都在这儿，深市基金走 15/16/18 开头，不冲突）
  const isSh =
    ["600", "601", "603", "605", "688", "900"].includes(p3) ||
    code.startsWith("5") ||
    ["10", "11"].includes(p2) // 国债/公司债/可转债
  if (isSh) return "SS"

  // 深圳证券交易所
  const isSz =
    [
      "000",
      "001",
      "002",
      "003",
      "004",
      "005",
      "006",
      "007",
      "008",
      "009", // 主板
      "300",
      "301", // 创业板
      "200", // B股
      "123",
      "127",
      "128",
      "129",
      "148",
      "149", // 可转债
    ].includes(p3) || ["15", "16", "18"].includes(p2) // 场内基金
  if (isSz) return "SZ"

  throw new Error(`无法推断交易所，代码 '${code}' 不在已知前缀范围内`)
}

/**
 * 将任意格式的 A 股代码统一为 Yahoo 风格 `XXXXXX.SS/.SZ/.BJ`。
 * 与后端 `to_yahoo_symbol`（crates/api/src/utils.rs）行为一致：
 * - 裸 6 位数字：按前缀推断交易所（600/688/513 → SS，000/300/001 → SZ，
 *   399 → SZ 指数，43/83/87/88/920/899 → BJ）
 * - 带 `.SS/.SZ/.BJ` 后缀：规范化代码后保留后缀（`000001.SS` 上证指数与
 *   `000001.SZ` 平安银行靠这个区分，不能丢）
 * - 带 `sh/sz/bj/ss` 前缀（含 `sh.600000`、`SZ-000001` 等变体）：按前缀定交易所
 * 非法输入抛 Error。
 */
export function toYahooSymbol(input: string): string {
  const trimmed = input.trim()

  // 1. 已是 Yahoo 格式（含 .SS / .SZ / .BJ / .HK / .TW）
  const dotIdx = trimmed.indexOf(".")
  if (dotIdx !== -1) {
    const code = trimmed.slice(0, dotIdx)
    const suffix = trimmed.slice(dotIdx + 1).toUpperCase()
    if (YAHOO_SUFFIXES.includes(suffix)) {
      return `${normalizeCode(code)}.${suffix}`
    }
  }

  // 2. 带交易所前缀，如 SH600000 / SZ000001 / sh.600000 / sz.000001
  const [exchange, rawCode] = parsePrefixed(trimmed)

  // 3. 规范化为 6 位纯数字
  const code = normalizeCode(rawCode)

  // 4. 确定交易所后缀
  let suffix: "SS" | "SZ" | "BJ"
  if (exchange) {
    const ex = exchange.toUpperCase()
    if (ex === "SH" || ex === "SS") suffix = "SS"
    else if (ex === "SZ") suffix = "SZ"
    else if (ex === "BJ") suffix = "BJ"
    else throw new Error(`未知交易所前缀: ${exchange}`)
  } else {
    suffix = inferExchange(code)
  }

  return `${code}.${suffix}`
}

/**
 * 标的类别。只有「个股」有固定涨跌停（分时网格要靠它），指数/基金/债券都没有
 * 能从代码推出来的固定幅度——`159915` 创业板ETF 是 ±20%、`159919` 沪深300ETF
 * 是 ±10%，号码段一模一样，所以只能按类别走不同的量程分支。
 */
export type SymbolKind = "stock" | "index" | "fund" | "bond"

/**
 * 从 Yahoo 代码判断标的类别。
 *
 * 号段基本按交易所分开，只有 `184xxx` 两市都在用（沪市企业债 / 深市基金），
 * 所以除了号码段还要看后缀：
 * - 指数：`399xxx`（深证）、`899xxx`（北证）、`.SS` 的 `000xxx`（上证）
 * - 基金：沪市 `5xxxxx` 整段，深市 `15/16/18xxxx`
 * - 债券：`10/11/12/13xxxx`（国债/企债/可转债/可交换债）、`148/149xxx`
 *
 * `000001` 裸码在上证指数与平安银行之间天然歧义，没有后缀时一律按个股算
 * （与 `inferExchange` 的默认一致）。
 */
export function classifySymbol(symbol: string): SymbolKind {
  const trimmed = symbol.trim().toUpperCase()
  const dot = trimmed.lastIndexOf(".")
  const prefix = /^(SH|SZ|BJ)/.exec(trimmed)?.[1]
  const code = dot === -1 ? trimmed.replace(/^(SH|SZ|BJ)/, "") : trimmed.slice(0, dot)
  const suffix = dot === -1 ? (prefix === "SH" ? "SS" : prefix) : trimmed.slice(dot + 1)
  if (!/^\d{6}$/.test(code)) return "stock"

  const p2 = code.slice(0, 2)
  const p3 = code.slice(0, 3)

  // 指数：399/899 裸码就能认，000xxx 必须带 .SS（否则是深市个股）
  if (p3 === "399" || p3 === "899") return "index"
  if (p3 === "000" && suffix === "SS") return "index"

  if (code.startsWith("5")) return "fund" // 沪市基金专用段（ETF / LOF / REIT）
  if (p2 === "15" || p2 === "16") return "fund" // 深市 ETF / LOF
  if (p2 === "18") return suffix === "SS" ? "bond" : "fund" // 184 两市都占

  if (["10", "11", "12", "13"].includes(p2)) return "bond"
  if (["148", "149"].includes(p3)) return "bond"
  return "stock"
}

/**
 * 归一化为 Yahoo 格式；失败时回退原始输入，与后端 watchlist 的容错保持一致。
 *
 * 交易所后缀必须透传：指数与个股共用 6 位代码（`000001.SS` 上证指数 vs
 * `000001.SZ` 平安银行），按首位数字重推会把指数画成另一只票。
 */
export function toUpstreamSymbol(raw: string): string {
  try {
    return toYahooSymbol(raw)
  } catch {
    return raw.trim()
  }
}

export function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf(".")
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    let suffix = raw.substring(dot + 1).toLowerCase()
    if (suffix === "ss") suffix = "sh"
    return suffix + code
  }
  return (
    (raw.startsWith("6") || raw.startsWith("9")
      ? "sh"
      : raw.startsWith("0") || raw.startsWith("3")
        ? "sz"
        : "bj") + raw
  )
}

export function getKlineImageUrl(symbol: string, period: string): string {
  const s = normalizeSymbol(symbol)
  switch (period) {
    case "minute":
      return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
    case "day":
      return `https://image.sinajs.cn/newchart/daily/n/${s}.gif`
    case "week":
      return `https://image.sinajs.cn/newchart/weekly/n/${s}.gif`
    case "month":
      return `https://image.sinajs.cn/newchart/monthly/n/${s}.gif`
    default:
      return `https://image.sinajs.cn/newchart/min/n/${s}.gif`
  }
}

// Fallback using Sina v5 format (newer, more reliable)
export function getKlineImageUrlV5(symbol: string, period: string): string {
  const s = normalizeSymbol(symbol)
  switch (period) {
    case "minute":
      return `https://image.sinajs.cn/newchart/v5/min/n/${s}.gif`
    case "day":
      return `https://image.sinajs.cn/newchart/v5/daily/n/${s}.gif`
    case "week":
      return `https://image.sinajs.cn/newchart/v5/weekly/n/${s}.gif`
    case "month":
      return `https://image.sinajs.cn/newchart/v5/monthly/n/${s}.gif`
    default:
      return `https://image.sinajs.cn/newchart/v5/min/n/${s}.gif`
  }
}
