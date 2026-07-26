// ── Native WebSocket agent transport hook ──────────────────────────────
//
// Replaces react-use-websocket with a native WebSocket implementation to
// avoid issues with null-to-string URL transitions in that library.
// Manages connection lifecycle, reconnection, and event processing.

import { useCallback, useEffect, useRef, useState } from "react"
import { applyEvent, historyToTurns } from "./reducer"
import type { HistoryMessage, TranscriptEvent, TranscriptTurn } from "./types"
import { fetchJson, getApiBaseUrl } from "@/stores/data"

export enum ReadyState {
  UNINSTANTIATED = -1,
  CONNECTING = 0,
  OPEN = 1,
  CLOSING = 2,
  CLOSED = 3,
}

function wsBaseUrl(): string {
  const base = getApiBaseUrl()
  if (base) {
    return base.replace(/^http:/, "ws:").replace(/^https:/, "wss:")
  }
  return `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}`
}

export interface UseAgentWebSocketOptions {
  /** Called when turns are updated with new events */
  onTurnsUpdate?: (turns: TranscriptTurn[]) => void
  /** Called when a terminal event (agent_end / notice) is received */
  onTerminal?: () => void
  /** Called when the backend signals `resync_required` (event buffer evicted
   *  the requested seq). The hook refetches the full message history itself,
   *  rebuilds turns, and calls `onTurnsUpdate`; this callback lets the host
   *  react (e.g. clear UI streaming state). */
  onResync?: () => void
}

export function useAgentWebSocket(
  sessionId: string | null,
  options: UseAgentWebSocketOptions = {},
) {
  const turnsRef = useRef<TranscriptTurn[]>([])
  const [readyState, setReadyState] = useState<ReadyState>(ReadyState.UNINSTANTIATED)
  const wsRef = useRef<WebSocket | null>(null)
  const readyResolveRef = useRef<(() => void) | null>(null)
  const reconnectCountRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const optionsRef = useRef(options)
  const mountedRef = useRef(true)

  // Keep options ref current. Done in an effect so we never mutate a ref
  // during render (react-hooks/refs rule).
  useEffect(() => {
    optionsRef.current = options
  }, [options])

  // Connect/disconnect WebSocket based on sessionId
  useEffect(() => {
    mountedRef.current = true

    // Close existing connection
    if (wsRef.current) {
      wsRef.current.onopen = null
      wsRef.current.onclose = null
      wsRef.current.onmessage = null
      wsRef.current.onerror = null
      if (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING) {
        wsRef.current.close()
      }
      wsRef.current = null
    }
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current)
      reconnectTimerRef.current = null
    }
    reconnectCountRef.current = 0

    if (!sessionId) return

    const url = `${wsBaseUrl()}/api/ai/sessions/${sessionId}/ws`
    console.log('[useAgentWebSocket] connecting to', url)

    const connect = () => {
      if (!mountedRef.current) return

      setReadyState(ReadyState.CONNECTING)

      const ws = new WebSocket(url)
      wsRef.current = ws

      ws.onopen = () => {
        if (!mountedRef.current || ws !== wsRef.current) return
        console.log('[useAgentWebSocket] connected', url)
        setReadyState(ReadyState.OPEN)
        reconnectCountRef.current = 0
        readyResolveRef.current?.()
        readyResolveRef.current = null
      }

      ws.onclose = () => {
        if (!mountedRef.current || ws !== wsRef.current) return
        console.log('[useAgentWebSocket] closed', url)
        setReadyState(ReadyState.CLOSED)

        // Auto-reconnect
        const maxAttempts = 20
        if (reconnectCountRef.current < maxAttempts) {
          const delay = Math.min(1000 * Math.pow(1.5, reconnectCountRef.current), 30000)
          reconnectCountRef.current++
          reconnectTimerRef.current = setTimeout(() => {
            if (mountedRef.current) connect()
          }, delay)
        }
      }

      ws.onerror = () => {
        console.error('[useAgentWebSocket] error', url)
      }

      ws.onmessage = (event: MessageEvent) => {
        if (!mountedRef.current || ws !== wsRef.current) return
        try {
          const parsed = JSON.parse(event.data) as Record<string, unknown>

          // Handle resync signal: the server-side event buffer has evicted the
          // requested `last_seq` (happens after a backend restart or a long
          // disconnection). Discard the partial in-memory turns and rebuild
          // the transcript from the persisted JSONL via REST.
          if (parsed.type === "resync_required") {
            const sid = sessionId
            // Clear the accumulated turns first so stale/incomplete state
            // does not bleed into the rebuilt transcript.
            turnsRef.current = []
            optionsRef.current.onTurnsUpdate?.([])
            fetchJson<HistoryMessage[]>(`/api/ai/sessions/${sid}/messages`)
              .then((msgs) => {
                const turns = msgs && msgs.length > 0 ? historyToTurns(msgs) : []
                turnsRef.current = turns
                optionsRef.current.onTurnsUpdate?.([...turns])
                optionsRef.current.onResync?.()
              })
              .catch((e) => {
                console.error("[useAgentWebSocket] resync fetch failed", e)
              })
            return
          }

          const prev = turnsRef.current
          const { turns: newTurns, terminal } = applyEvent(prev, parsed as TranscriptEvent)
          turnsRef.current = newTurns
          optionsRef.current.onTurnsUpdate?.([...newTurns])
          if (terminal) {
            optionsRef.current.onTerminal?.()
          }
        } catch {
          // Ignore parse errors for malformed messages
        }
      }
    }

    connect()

    return () => {
      mountedRef.current = false
      if (wsRef.current) {
        wsRef.current.onopen = null
        wsRef.current.onclose = null
        wsRef.current.onmessage = null
        wsRef.current.onerror = null
        if (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING) {
          wsRef.current.close()
        }
        wsRef.current = null
      }
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current)
        reconnectTimerRef.current = null
      }
    }
  }, [sessionId])

  // Exposed connection state. When there is no active session the hook reports
  // UNINSTANTIATED regardless of any lingering ws state, which replaces the
  // previous synchronous reset setState.
  const effectiveReadyState =
    sessionId !== null ? readyState : ReadyState.UNINSTANTIATED
  const isReady = effectiveReadyState === ReadyState.OPEN

  /** Wait for the WebSocket connection to be open */
  const waitForReady = useCallback(async (): Promise<void> => {
    if (effectiveReadyState === ReadyState.OPEN) return
    return new Promise<void>((resolve) => {
      readyResolveRef.current = resolve
    })
  }, [effectiveReadyState])

  /** Send a user prompt to the agent */
  const sendPrompt = useCallback(
    async (text: string): Promise<void> => {
      await waitForReady()
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: "prompt", text }))
      }
    },
    [waitForReady],
  )

  /** Interrupt the running agent turn */
  const interrupt = useCallback(async (): Promise<void> => {
    await waitForReady()
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: "interrupt" }))
    }
  }, [waitForReady])

  /** Respond to a permission request */
  const respondToPermission = useCallback(
    async (id: string, approved: boolean): Promise<void> => {
      await waitForReady()
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: "permission_response", id, approved }))
      }
    },
    [waitForReady],
  )

  /** Reset the internal turns state */
  const reset = useCallback(() => {
    turnsRef.current = []
  }, [])

  return {
    sendPrompt,
    interrupt,
    respondToPermission,
    isReady,
    readyState: effectiveReadyState,
    reset,
  }
}
