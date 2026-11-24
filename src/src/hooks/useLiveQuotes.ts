import { useEffect, useState } from "react"
import { subscribeRealtimeBatch, type RealtimeQuote } from "@/hooks/useRealtimeStream"

/** 榜单里会被实时覆盖的两个字段。 */
export interface LiveQuote {
  price: number | null
  pct: number | null
}

/**
 * 只在可视区订阅实时报价。
 *
 * 榜单的"有谁、什么顺序"由慢节奏快照决定（见 `useStockRanking`），这里只负责
 * 把可见那几十行的最新价/涨跌幅刷起来。
 */
export function useLiveQuotes(codes: string[]): Map<string, LiveQuote> {
  const [quotes, setQuotes] = useState<Map<string, LiveQuote>>(new Map())
  // codes 每次渲染都是新数组，用拼接出来的字符串当依赖，内容变才重订阅
  const key = codes.join(",")

  useEffect(() => {
    const list = key ? key.split(",") : []
    if (list.length === 0) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- 没有订阅目标时清空上一轮行情
      setQuotes(new Map())
      return
    }
    const wanted = new Set(list)
    // 换可视区时把已经滚走的代码清掉：不清会随会话时长无限增长
    setQuotes((prev) => {
      const next = new Map<string, LiveQuote>()
      for (const code of list) {
        const quote = prev.get(code)
        if (quote) next.set(code, quote)
      }
      return next
    })
    return subscribeRealtimeBatch<RealtimeQuote>(list, (code, data) => {
      if (!wanted.has(code)) return
      setQuotes((prev) => {
        const next = new Map(prev)
        next.set(code, {
          price: typeof data.last_px === "number" && data.last_px > 0 ? data.last_px : null,
          pct: typeof data.px_change_rate === "number" ? data.px_change_rate : null,
        })
        return next
      })
    })
  }, [key])

  return quotes
}
