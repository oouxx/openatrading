// Shared stock symbol and chart URL utilities

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
