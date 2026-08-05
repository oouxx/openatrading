/**
 * K线 GIF 图片回退组件（sina 源）
 *
 * 当 KlineChart（lightweight-charts）不可用时，可用此组件替代
 * 显示新浪图片源生成的静态 K 线图。
 *
 * 使用方式：
 *   import KlineImageFallback from "./KlineImageFallback"
 *   // 替换 <KlineChart /> 为 <KlineImageFallback symbol={s} period={p} name={n} />
 *
 * 数据源：
 *   https://image.sinajs.cn/newchart/{period}/n/{normalizedSymbol}.gif
 */

function normalizeSymbol(raw: string): string {
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

function getKlineImageUrl(symbol: string, period: string): string {
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

function getKlineImageUrlV5(symbol: string, period: string): string {
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

interface KlineImageFallbackProps {
  symbol: string
  period: string
  name: string
  /** 使用 V5 版本（新版稳定接口），默认 true */
  v5?: boolean
}

export default function KlineImageFallback({
  symbol,
  period,
  name,
  v5 = true,
}: KlineImageFallbackProps) {
  const url = v5 ? getKlineImageUrlV5(symbol, period) : getKlineImageUrl(symbol, period)
  return (
    <img
      src={url}
      alt={`${name} K线图`}
      className="rounded-lg w-full"
      style={{ minHeight: 300, maxHeight: 420 }}
    />
  )
}
