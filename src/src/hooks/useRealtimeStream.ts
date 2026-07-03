const isTauri = "__TAURI_INTERNALS__" in window

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
  onError?: (err: string) => void,
): () => void {
  if (!symbol) return () => {}

  if (isTauri) {
    let active = true
    ;(async () => {
      const { invoke, Channel } = await import("@tauri-apps/api/core")
      const channel = new Channel<string>()
      channel.onmessage = (text) => {
        if (!active) return
        try {
          const parsed = JSON.parse(text)
          if (parsed.error) {
            onError?.(parsed.error)
          } else {
            onData(parsed)
          }
        } catch {
          /* ignore parse errors */
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
  const base =
    localStorage.getItem("openatrading-api-url") || __API_BASE_URL__
  const es = new EventSource(`${base}/api/realtime/stream?symbols=${encodeURIComponent(symbol)}`)
  es.onmessage = (e) => {
    try {
      const data = JSON.parse(e.data)
      if (data.error) {
        onError?.(data.error)
      } else {
        onData(data)
      }
    } catch {
      /* ignore */
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    es.close()
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
  onError?: (err: string) => void,
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
        try {
          const parsed = JSON.parse(text)
          if (parsed.error) {
            onError?.(parsed.error)
          } else {
            // parsed is { "000001.SZ": {...}, ... }
            for (const rawCode of symbols) {
              // Try exact key first, then look for symbol matching rawCode
              let found: unknown = parsed[rawCode]
              if (!found) {
                // Try matching by stripping .SS/.SZ suffix
                for (const [k, v] of Object.entries(parsed)) {
                  if (k.replace(/\.(SS|SZ|BJ)$/i, "") === rawCode) {
                    found = v
                    break
                  }
                }
              }
              if (found) {
                onData(rawCode, found)
              }
            }
          }
        } catch {
          /* ignore parse errors */
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
  const base =
    localStorage.getItem("openatrading-api-url") || __API_BASE_URL__
  const es = new EventSource(`${base}/api/realtime/stream?symbols=${encodeURIComponent(code)}`)
  es.onmessage = (e) => {
    try {
      const parsed = JSON.parse(e.data)
      if (parsed.error) {
        onError?.(parsed.error)
      } else {
        // parsed is { "000001.SZ": {...}, ... }
        for (const rawCode of symbols) {
          let found: unknown = parsed[rawCode]
          if (!found) {
            for (const [k, v] of Object.entries(parsed)) {
              if (k.replace(/\.(SS|SZ|BJ)$/i, "") === rawCode) {
                found = v
                break
              }
            }
          }
          if (found) {
            onData(rawCode, found)
          }
        }
      }
    } catch {
      /* ignore */
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    es.close()
  }
  return () => es.close()
}

function sseStream(
  urlPath: string,
  onData: (data: unknown) => void,
  onError?: (err: string) => void,
): () => void {
  const base =
    localStorage.getItem("openatrading-api-url") || __API_BASE_URL__
  const es = new EventSource(`${base}${urlPath}`)
  es.onmessage = (e) => {
    try {
      const data = JSON.parse(e.data)
      if (data.error) {
        onError?.(data.error)
      } else {
        onData(data)
      }
    } catch {
      /* ignore */
    }
  }
  es.onerror = () => {
    onError?.("connection error")
    es.close()
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
  onError?: (err: string) => void,
): () => void {
  if (!symbol) return () => {}

  if (isTauri) {
    let active = true
    ;(async () => {
      const { invoke, Channel } = await import("@tauri-apps/api/core")
      const channel = new Channel<string>()
      channel.onmessage = (text) => {
        if (!active) return
        try {
          const parsed = JSON.parse(text)
          if (parsed.error) {
            onError?.(parsed.error)
          } else {
            onData(parsed)
          }
        } catch {
          /* ignore parse errors */
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

  return sseStream(
    `/api/kline/stream?symbols=${encodeURIComponent(symbol)}&period=${period}&count=${count}`,
    onData,
    onError,
  )
}
