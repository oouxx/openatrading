const isTauri = "__TAURI_INTERNALS__" in window

/**
 * Subscribe to realtime quotes, updated every 3 seconds.
 *
 * - Web: uses EventSource (SSE) via /api/realtime/:symbol/stream
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
  const es = new EventSource(`${base}/api/realtime/${symbol}/stream`)
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
