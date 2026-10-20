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

import { getKlineImageUrl, getKlineImageUrlV5 } from "@/lib/stock-utils"

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
      className="w-full rounded-lg"
      style={{ minHeight: 300, maxHeight: 420 }}
    />
  )
}
