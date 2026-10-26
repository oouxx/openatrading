import { useEffect, useState } from "react"
import { subscribeKline, subscribeOrderBook } from "@/hooks/useRealtimeStream"
import type { KlineResponse, OrderBook } from "@/stores/data"

type StreamSnapshot<T> = { symbol: string; data: T | null; error: string | null }

/**
 * 盘口五档。与 K 线共用同一条行情通道（Web 走 SSE、桌面端走 IPC Channel），
 * 交易时段每 3 秒一帧，休市时后端把节奏降到 30 秒。
 *
 * 用订阅而不是轮询：下单弹窗开着的时候盘口必须跟着走；多个客户端看同一只票时
 * 后端由 MarketStreamHub 合并成一条上游轮询。
 *
 * 快照带上 `symbol`：换股票时旧数据在渲染层直接失效（`snapshot.symbol !== symbol`），
 * 不需要在 effect 里同步 setState 清空——那会多一轮渲染，也容易被 lint 拦下。
 */
export function useOrderBook(symbol: string) {
  const [snapshot, setSnapshot] = useState<StreamSnapshot<OrderBook>>({
    symbol,
    data: null,
    error: null,
  })

  useEffect(() => {
    if (!symbol) return
    // 行情流只在有数据时推帧，上游失败不会给客户端任何事件；没有这个超时，
    // 弹窗会永远停在骨架屏。8 秒拿不到第一帧就明确告诉用户取不到盘口。
    let timer: ReturnType<typeof setTimeout> | null = setTimeout(() => {
      setSnapshot({ symbol, data: null, error: "盘口加载超时（可能非交易时段或接口不可用）" })
    }, 8_000)
    const unsubscribe = subscribeOrderBook<OrderBook>(
      symbol,
      (data) => {
        if (timer) {
          clearTimeout(timer)
          timer = null
        }
        setSnapshot({ symbol, data, error: null })
      },
      (error) => {
        if (timer) {
          clearTimeout(timer)
          timer = null
        }
        setSnapshot({ symbol, data: null, error })
      }
    )
    return () => {
      if (timer) clearTimeout(timer)
      unsubscribe()
    }
  }, [symbol])

  const current = snapshot.symbol === symbol ? snapshot : null
  return { book: current?.data ?? null, error: current?.error ?? null }
}

/** 下单弹窗里的当日分时（1 分钟 × 240 根 ≈ 一个交易日），同样走行情通道。 */
export function useIntradayCandles(symbol: string) {
  const [snapshot, setSnapshot] = useState<StreamSnapshot<KlineResponse["candles"]>>({
    symbol,
    data: null,
    error: null,
  })

  useEffect(() => {
    if (!symbol) return
    return subscribeKline<KlineResponse>(
      symbol,
      "minute",
      240,
      (data) => setSnapshot({ symbol, data: data.candles ?? [], error: null }),
      (error) => setSnapshot({ symbol, data: null, error })
    )
  }, [symbol])

  const current = snapshot.symbol === symbol ? snapshot : null
  return { candles: current?.data ?? [], error: current?.error ?? null }
}
