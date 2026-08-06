import { getApiBaseUrl } from "@/stores/data"

import { isTauri } from "@/lib/env"
import { toYahooSymbol } from "@/lib/stock-utils"

/** 归一化失败时回退原始输入，保持与后端容错行为一致。 */
function normalizeSymbolSafe(raw: string): string {
  try {
    return toYahooSymbol(raw)
  } catch {
    return raw
  }
}

/**
 * Try to match a raw symbol (e.g. "000001") against a batch-response map
 * whose keys may carry market suffixes (e.g. "000001.SZ").
 */
function matchSymbol(parsed: Record<string, unknown>, rawCode: string): unknown {
  if (rawCode in parsed) return parsed[rawCode]
  for (const [k, v] of Object.entries(parsed)) {
    if (k.replace(/\.(SS|SZ|BJ)$/i, "") === rawCode) return v
  }
  return undefined
}

function safeParse(raw: string, context: string): Record<string, unknown> | null {
  try {
    return JSON.parse(raw)
  } catch (e) {
    console.warn(`[${context}] failed to parse SSE payload:`, raw, e)
    return null
  }
}

/**
 * Subscribe to realtime quotes, updated every 3 seconds.
 *
 * - Web: uses EventSource (SSE) via /api/realtime/stream?symbols=...
 * - Tauri: uses Channel via api_realtime_stream
 *
 * Returns a cleanup function — call it to unsubscribe.
 */
export function subscribeRealtime(
  symbol: string,
  onData: (data: unknown) => void,
  onError?: (err: string) => void
): () => void {
  if (!symbol) return () => {}
  const normalized = normalizeSymbolSafe(symbol)

  if (isTauri) {
    let active = true
    ;(async () => {
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
          const found = matchSymbol(parsed, normalized) ?? Object.values(parsed)[0]
          if (found) onData(found)
        }
      }
      try {
        await invoke("api_realtime_stream", { symbol: normalized, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    })()
    return () => {
      active = false
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
      const found = matchSymbol(parsed, normalized) ?? Object.values(parsed)[0]
      if (found) onData(found)
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
export function subscribeRealtimeBatch(
  symbols: string[],
  onData: (code: string, data: unknown) => void,
  onError?: (err: string) => void
): () => void {
  if (symbols.length === 0) return () => {}

  const normalizedSymbols = symbols.map(normalizeSymbolSafe)
  const code = normalizedSymbols.join(",")

  if (isTauri) {
    let active = true
    ;(async () => {
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
            const found = matchSymbol(parsed, normalizedSymbols[i])
            if (found) onData(symbols[i], found)
          }
        }
      }
      try {
        await invoke("api_realtime_stream", { symbol: code, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    })()
    return () => {
      active = false
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
        const found = matchSymbol(parsed, normalizedSymbols[i])
        if (found) onData(symbols[i], found)
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
 * Subscribe to kline data via SSE, updated every 3 seconds.
 * - Web: EventSource at /api/kline/stream?symbols=X&period=X&count=256
 * - Tauri: Channel via api_kline_stream
 *
 * Returns a cleanup function.
 */
export function subscribeKline(
  symbol: string,
  period: string,
  count: number,
  onData: (data: unknown) => void,
  onError?: (err: string) => void
): () => void {
  if (!symbol) return () => {}

  if (isTauri) {
    let active = true
    ;(async () => {
      const { invoke, Channel } = await import("@tauri-apps/api/core")
      const channel = new Channel<string>()
      channel.onmessage = (text) => {
        if (!active) return
        const parsed = safeParse(text, "kline")
        if (!parsed) return
        if (parsed.error) {
          onError?.(String(parsed.error))
        } else {
          onData(parsed)
        }
      }
      try {
        await invoke("api_kline_stream", { symbol, period, count, channel })
      } catch (e: unknown) {
        if (active) onError?.(String(e))
      }
    })()
    return () => {
      active = false
    }
  }

  // Web: sseStream passes raw JSON — unwrap batch map to extract single symbol data
  const wrappedOnData = (data: unknown) => {
    const map = data as Record<string, unknown>
    const found = matchSymbol(map, symbol) ?? Object.values(map)[0]
    if (found) onData(found)
  }

  return sseStream(
    `/api/kline/stream?symbols=${encodeURIComponent(symbol)}&period=${period}&count=${count}`,
    wrappedOnData,
    onError
  )
}
