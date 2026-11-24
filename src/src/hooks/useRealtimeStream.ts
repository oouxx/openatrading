import { getApiBaseUrl } from "@/stores/data"

import { isTauri } from "@/lib/env"
import { toYahooSymbol } from "@/lib/stock-utils"
import { pickSymbolPayload } from "@/lib/stream-payload"

/** 归一化失败时回退原始输入，保持与后端容错行为一致。 */
function normalizeSymbolSafe(raw: string): string {
  try {
    return toYahooSymbol(raw)
  } catch {
    return raw
  }
}

function safeParse(raw: string, context: string): Record<string, unknown> | null {
  try {
    return JSON.parse(raw)
  } catch (e) {
    console.warn(`[${context}] failed to parse SSE payload:`, raw, e)
    return null
  }
}

// Desktop polls the one-shot IPC stream command on the same cadence the
// backend used to push (3s). Keeps no long-lived invoke/channel alive.
const TAURI_STREAM_POLL_MS = 3000

/** 实时行情（后端 RealtimeQuote）。 */
export interface RealtimeQuote {
  last_px?: number
  preclose_px?: number
  open_px?: number
  high_px?: number
  low_px?: number
  px_change_rate?: number
  amplitude?: number
  turnover_ratio?: number
  dyn_pe?: number
  dyn_pb_rate?: number
  market_value?: number
  circulation_value?: number
  volume_ratio?: number
  turnover_volume?: number
}

/**
 * Subscribe to realtime quotes, updated every 3 seconds.
 *
 * - Web: uses EventSource (SSE) via /api/realtime/stream?symbols=...
 * - Tauri: uses Channel via api_realtime_stream
 *
 * Returns a cleanup function — call it to unsubscribe.
 */
export function subscribeRealtime<T = unknown>(
  symbol: string,
  onData: (data: T) => void,
  onError?: (err: string) => void
): () => void {
  if (!symbol) return () => {}
  const normalized = normalizeSymbolSafe(symbol)

  if (isTauri) {
    let active = true
    let timer: ReturnType<typeof setInterval> | null = null
    const poll = async () => {
      if (!active) return
      try {
        const { invoke, Channel } = await import("@tauri-apps/api/core")
        const channel = new Channel<string>()
        channel.onmessage = (text) => {
          if (!active) return
          const parsed = safeParse(text, "realtime")
          if (!parsed) return
          if (parsed.error) {
            onError?.(String(parsed.error))
          } else {
            // Backend returns { "symbol": {...}, ... } — extract the one we asked for
            const found = pickSymbolPayload(parsed, normalized)
            if (found) onData(found as T)
          }
        }
        await invoke("api_realtime_stream", { symbol: normalized, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    }
    poll()
    timer = setInterval(poll, TAURI_STREAM_POLL_MS)
    return () => {
      active = false
      if (timer) clearInterval(timer)
    }
  }

  // Web: EventSource
  const es = new EventSource(
    `${getApiBaseUrl()}/api/realtime/stream?symbols=${encodeURIComponent(normalized)}`
  )
  es.onmessage = (e) => {
    const parsed = safeParse(e.data, "realtime")
    if (!parsed) return
    if (parsed.error) {
      onError?.(String(parsed.error))
    } else {
      // Backend returns { "symbol": {...}, ... } — extract the one we asked for
      const found = pickSymbolPayload(parsed, normalized)
      if (found) onData(found as T)
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    if (es.readyState === EventSource.CLOSED) {
      es.close()
    }
  }
  return () => es.close()
}

/**
 * Subscribe to realtime quotes for MULTIPLE symbols via a single SSE connection.
 *
 * The response is a map `{ yahooSymbol: RealtimeQuote, ... }`.
 * `onData` is called with a `{ code, data }` pair for each symbol in `symbols`
 * (where `code` is the original raw symbol passed in).
 *
 * Returns a cleanup function.
 */
export function subscribeRealtimeBatch<T = unknown>(
  symbols: string[],
  onData: (code: string, data: T) => void,
  onError?: (err: string) => void
): () => void {
  if (symbols.length === 0) return () => {}

  // 一条连接带全部代码会把 URL 推过代理的长度上限（自选一多就是 414），
  // 按批次拆成多条连接：每条约 1KB，几秒钟一轮也不会互相拖慢。
  const unique = [...new Set(symbols)]
  if (unique.length <= REALTIME_BATCH_SIZE) {
    return subscribeRealtimeBatchChunk(unique, onData, onError)
  }
  const cleanups: (() => void)[] = []
  for (let i = 0; i < unique.length; i += REALTIME_BATCH_SIZE) {
    cleanups.push(
      subscribeRealtimeBatchChunk(unique.slice(i, i + REALTIME_BATCH_SIZE), onData, onError)
    )
  }
  return () => {
    for (const cleanup of cleanups) cleanup()
  }
}

/** 一条连接承载的代码数上限（与服务端上游分片一致，别超过 60 太多）。 */
const REALTIME_BATCH_SIZE = 60

function subscribeRealtimeBatchChunk<T = unknown>(
  symbols: string[],
  onData: (code: string, data: T) => void,
  onError?: (err: string) => void
): () => void {
  if (symbols.length === 0) return () => {}

  const normalizedSymbols = symbols.map(normalizeSymbolSafe)
  const code = normalizedSymbols.join(",")

  if (isTauri) {
    let active = true
    let timer: ReturnType<typeof setInterval> | null = null
    const poll = async () => {
      if (!active) return
      try {
        const { invoke, Channel } = await import("@tauri-apps/api/core")
        const channel = new Channel<string>()
        channel.onmessage = (text) => {
          if (!active) return
          const parsed = safeParse(text, "realtime-batch")
          if (!parsed) return
          if (parsed.error) {
            onError?.(String(parsed.error))
          } else {
            for (let i = 0; i < symbols.length; i++) {
              const found = pickSymbolPayload(parsed, normalizedSymbols[i])
              if (found) onData(symbols[i], found as T)
            }
          }
        }
        await invoke("api_realtime_stream", { symbol: code, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    }
    // Backend `api_realtime_stream` is one-shot, so we must re-invoke on the
    // same cadence as the single-symbol `subscribeRealtime` to keep the batch
    // (watchlist) quotes live on desktop.
    poll()
    timer = setInterval(poll, TAURI_STREAM_POLL_MS)
    return () => {
      active = false
      if (timer) clearInterval(timer)
    }
  }

  // Web: EventSource
  const es = new EventSource(
    `${getApiBaseUrl()}/api/realtime/stream?symbols=${encodeURIComponent(code)}`
  )
  es.onmessage = (e) => {
    const parsed = safeParse(e.data, "realtime-batch")
    if (!parsed) return
    if (parsed.error) {
      onError?.(String(parsed.error))
    } else {
      for (let i = 0; i < symbols.length; i++) {
        const found = pickSymbolPayload(parsed, normalizedSymbols[i])
        if (found) onData(symbols[i], found as T)
      }
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    if (es.readyState === EventSource.CLOSED) {
      es.close()
    }
  }
  return () => es.close()
}

function sseStream(
  urlPath: string,
  onData: (data: unknown) => void,
  onError?: (err: string) => void
): () => void {
  const es = new EventSource(`${getApiBaseUrl()}${urlPath}`)

  es.onmessage = (e) => {
    const data = safeParse(e.data, urlPath)
    if (!data) return
    if (data.error) {
      onError?.(String(data.error))
    } else {
      onData(data)
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    if (es.readyState === EventSource.CLOSED) {
      es.close()
    }
  }
  return () => es.close()
}

/**
 * 订阅盘口五档，与 K 线用同一条通道（交易时段每 3 秒一帧）。
 * - Web: EventSource at `/api/order-book/stream?symbols=X`
 * - Tauri: `api_order_book_stream` 的 Channel，按同一节奏重新 invoke
 *
 * 返回清理函数。
 */
export function subscribeOrderBook<T = unknown>(
  symbol: string,
  onData: (data: T) => void,
  onError?: (err: string) => void
): () => void {
  if (!symbol) return () => {}

  if (isTauri) {
    let active = true
    let timer: ReturnType<typeof setInterval> | null = null
    const poll = async () => {
      if (!active) return
      try {
        const { invoke, Channel } = await import("@tauri-apps/api/core")
        const channel = new Channel<string>()
        channel.onmessage = (text) => {
          if (!active) return
          const parsed = safeParse(text, "order-book")
          if (!parsed) return
          if (parsed.error) {
            onError?.(String(parsed.error))
          } else {
            // 批量形状 { "000001": OrderBook }；payload 是全站订阅的并集，
            // 匹配不到自己那只时**不能**退回第一条（那会是别人的盘口）
            const found = pickSymbolPayload(parsed, symbol)
            if (found) onData(found as T)
          }
        }
        await invoke("api_order_book_stream", { symbol, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    }
    poll()
    timer = setInterval(poll, TAURI_STREAM_POLL_MS)
    return () => {
      active = false
      if (timer) clearInterval(timer)
    }
  }

  return sseStream(
    `/api/order-book/stream?symbols=${encodeURIComponent(symbol)}`,
    (data) => {
      const map = data as Record<string, unknown>
      // 并集 payload：只认自己这只，匹配不到就等下一帧（绝不退回第一条）
      const found = pickSymbolPayload(map, symbol)
      if (found) onData(found as T)
    },
    onError
  )
}

/**
 * Subscribe to kline data via SSE, updated every 3 seconds.
 * - Web: EventSource at /api/kline/stream?symbols=X&period=X&count=256
 * - Tauri: Channel via api_kline_stream
 *
 * Returns a cleanup function.
 */
export function subscribeKline<T = unknown>(
  symbol: string,
  period: string,
  count: number,
  onData: (data: T) => void,
  onError?: (err: string) => void
): () => void {
  if (!symbol) return () => {}

  if (isTauri) {
    let active = true
    let timer: ReturnType<typeof setInterval> | null = null
    let warned = false
    const poll = async () => {
      if (!active) return
      try {
        const { invoke, Channel } = await import("@tauri-apps/api/core")
        const channel = new Channel<string>()
        channel.onmessage = (text) => {
          if (!active) return
          const parsed = safeParse(text, "kline")
          if (!parsed) return
          if (parsed.error) {
            onError?.(String(parsed.error))
          } else {
            // 与 Web 端一致：接口返回 { "600519.SS": <kline> }，按 symbol 取出来
            const found = pickSymbolPayload(parsed, symbol)
            if (found) onData(found as T)
            else if (!warned) {
              warned = true
              console.warn("[kline] payload 里没有这只票", symbol, Object.keys(parsed))
            }
          }
        }
        await invoke("api_kline_stream", { symbol, period, count, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    }
    poll()
    timer = setInterval(poll, TAURI_STREAM_POLL_MS)
    return () => {
      active = false
      if (timer) clearInterval(timer)
    }
  }

  // Web: sseStream passes raw JSON — unwrap batch map to extract single symbol data
  let warned = false
  const wrappedOnData = (data: unknown) => {
    const map = data as Record<string, unknown>
    const found = pickSymbolPayload(map, symbol)
    if (found) onData(found as T)
    else if (!warned) {
      warned = true
      console.warn("[kline] payload 里没有这只票", symbol, Object.keys(map))
    }
  }

  return sseStream(
    `/api/kline/stream?symbols=${encodeURIComponent(symbol)}&period=${period}&count=${count}`,
    wrappedOnData,
    onError
  )
}
