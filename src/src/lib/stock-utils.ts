// Shared stock symbol and chart URL utilities

// ── Symbol normalization (mirrors backend crates/api/src/utils.rs to_yahoo_symbol) ──

const YAHOO_SUFFIXES = ["SS", "SZ", "HK", "TW"]

function parsePrefixed(input: string): [string | null, string] {
  const upper = input.toUpperCase()
  for (const prefix of ["SH", "SZ", "SS"]) {
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

  // 北交所：83/87/88 开头的存量代码，920 开头为 2023 年后新股代码段
  if (["83", "87", "88"].includes(p2) || p3 === "920") return "BJ"

  // 上海证券交易所
  const isSh =
    [
      "600", "601", "603", "605", // 主板
      "688", // 科创板
      "900", // B股
      "510", "511", "512", "513", "515", "516", "517", "518", "519", "520", "588", // ETF / LOF
      "500", "501", "502", // 其他场内基金
    ].includes(p3) ||
    ["10", "11"].includes(p2) // 国债/公司债/可转债
  if (isSh) return "SS"

  // 深圳证券交易所
  const isSz = [
    "000", "001", "002", "003", "004", "005", "006", "007", "008", "009", // 主板
    "300", "301", // 创业板
    "200", // B股
    "150", "151", "152", "159", "160", "161", "162", "163", "164", "165", "166", "167", "168", "184", // ETF / LOF
    "123", "127", "128", "129", "148", "149", // 可转债
  ].includes(p3)
  if (isSz) return "SZ"

  throw new Error(`无法推断交易所，代码 '${code}' 不在已知前缀范围内`)
}

/**
 * 将任意格式的 A 股代码统一为 Yahoo 风格 `XXXXXX.SS/.SZ/.BJ`。
 * 与后端 `to_yahoo_symbol`（crates/api/src/utils.rs）行为一致：
 * - 裸 6 位数字：按前缀推断交易所（600/688/513 → SS，000/300/001 → SZ）
 * - 带 `.SS/.SZ` 后缀：规范化代码后保留后缀
 * - 带 `sh/sz/ss` 前缀（含 `sh.600000`、`SZ-000001` 等变体）：按前缀定交易所
 * 非法输入抛 Error。
 */
export function toYahooSymbol(input: string): string {
  const trimmed = input.trim()

  // 1. 已是 Yahoo 格式（含 .SS / .SZ / .HK / .TW）
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
    else throw new Error(`未知交易所前缀: ${exchange}`)
  } else {
    suffix = inferExchange(code)
  }

  return `${code}.${suffix}`
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
