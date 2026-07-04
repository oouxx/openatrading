const isTauri = "__TAURI_INTERNALS__" in window

function getApiBase(): string {
  return localStorage.getItem("openatrading-api-url") || __API_BASE_URL__
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
          onData(parsed)
        }
      }
      try {
        await invoke("api_realtime_stream", { symbol, channel })
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
    `${getApiBase()}/api/realtime/stream?symbols=${encodeURIComponent(symbol)}`
  )
  es.onmessage = (e) => {
    const parsed = safeParse(e.data, "realtime")
    if (!parsed) return
    if (parsed.error) {
      onError?.(String(parsed.error))
    } else {
      // Backend returns { "symbol": {...}, ... } — extract the one we asked for
      const found = matchSymbol(parsed, symbol) ?? Object.values(parsed)[0]
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

  const code = symbols.join(",")

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
          for (const rawCode of symbols) {
            const found = matchSymbol(parsed, rawCode)
            if (found) onData(rawCode, found)
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
    `${getApiBase()}/api/realtime/stream?symbols=${encodeURIComponent(code)}`
  )
  es.onmessage = (e) => {
    const parsed = safeParse(e.data, "realtime-batch")
    if (!parsed) return
    if (parsed.error) {
      onError?.(String(parsed.error))
    } else {
      for (const rawCode of symbols) {
        const found = matchSymbol(parsed, rawCode)
        if (found) onData(rawCode, found)
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
  const es = new EventSource(`${getApiBase()}${urlPath}`)

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
