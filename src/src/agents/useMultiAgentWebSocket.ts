// ── Multi-session WebSocket agent transport hook ─────────────────────────
//
// Manages multiple WebSocket connections simultaneously, one per active
// session. Each connection independently processes events into its own turn
// list. Connections are created/removed as the sessionIds array changes.

import { useCallback, useEffect, useRef } from "react"
import { applyEvent } from "./reducer"
import type { TranscriptEvent, TranscriptTurn } from "./types"
import { getApiBaseUrl } from "@/stores/data"

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

// ── Internal connection state ────────────────────────────────────────────

interface ConnectionState {
  ws: WebSocket | null
  turns: TranscriptTurn[]
  readyState: ReadyState
  reconnectCount: number
  reconnectTimer: ReturnType<typeof setTimeout> | null
  readyResolve: (() => void) | null
}

function createConnectionState(): ConnectionState {
  return {
    ws: null,
    turns: [],
    readyState: ReadyState.UNINSTANTIATED,
    reconnectCount: 0,
    reconnectTimer: null,
    readyResolve: null,
  }
}

// ── Hook ─────────────────────────────────────────────────────────────────

export interface UseMultiAgentWebSocketOptions {
  /** Called when any session's turns are updated */
  onTurnsUpdate?: (sessionId: string, turns: TranscriptTurn[]) => void
  /** Called when a terminal event (agent_end / notice) is received for a session */
  onTerminal?: (sessionId: string) => void
}

export function useMultiAgentWebSocket(
  sessionIds: string[],
  options: UseMultiAgentWebSocketOptions = {},
) {
  const connectionsRef = useRef<Map<string, ConnectionState>>(new Map())
  const optionsRef = useRef(options)
  const mountedRef = useRef(true)

  // Keep options ref current
  useEffect(() => {
    optionsRef.current = options
  }, [options])

  // Sync connections with sessionIds on every render (compares internally).
  // We use a layout-effect-like pattern: run after every render, compare
  // current sessionIds with previous ones stored in a ref.
  useEffect(() => {
    mountedRef.current = true
    const connections = connectionsRef.current
    const desired = new Set(sessionIds)

    // Disconnect sessions no longer in the list
    for (const [sid, conn] of connections) {
      if (!desired.has(sid)) {
        teardownConnection(sid, conn)
        connections.delete(sid)
      }
    }

    // Connect new sessions
    for (const sid of desired) {
      if (!connections.has(sid)) {
        const conn = createConnectionState()
        connections.set(sid, conn)
        connectSession(sid, conn)
      }
    }

    return () => {
      mountedRef.current = false
      for (const [sid, conn] of connections) {
        teardownConnection(sid, conn)
      }
      connections.clear()
    }
    // Intentionally no deps — runs on every render to detect sessionIds changes.
    // The function compares current vs previous sessionIds internally.
  })

  // ── Connection lifecycle helpers ──────────────────────────────────────

  function teardownConnection(_sid: string, conn: ConnectionState) {
    if (conn.ws) {
      conn.ws.onopen = null
      conn.ws.onclose = null
      conn.ws.onmessage = null
      conn.ws.onerror = null
      if (conn.ws.readyState === WebSocket.OPEN || conn.ws.readyState === WebSocket.CONNECTING) {
        conn.ws.close()
      }
      conn.ws = null
    }
    if (conn.reconnectTimer) {
      clearTimeout(conn.reconnectTimer)
      conn.reconnectTimer = null
    }
    conn.readyState = ReadyState.CLOSED
  }

  function connectSession(sid: string, conn: ConnectionState) {
    if (!mountedRef.current) return

    const url = `${wsBaseUrl()}/api/ai/sessions/${sid}/ws`
    conn.readyState = ReadyState.CONNECTING

    const ws = new WebSocket(url)
    conn.ws = ws

    ws.onopen = () => {
      if (!mountedRef.current || ws !== conn.ws) return
      conn.readyState = ReadyState.OPEN
      conn.reconnectCount = 0
      conn.readyResolve?.()
      conn.readyResolve = null
    }

    ws.onclose = () => {
      if (!mountedRef.current || ws !== conn.ws) return
      conn.readyState = ReadyState.CLOSED

      // Auto-reconnect with exponential backoff
      const maxAttempts = 20
      if (conn.reconnectCount < maxAttempts) {
        const delay = Math.min(1000 * Math.pow(1.5, conn.reconnectCount), 30000)
        conn.reconnectCount++
        conn.reconnectTimer = setTimeout(() => {
          if (mountedRef.current) connectSession(sid, conn)
        }, delay)
      }
    }

    ws.onerror = () => {
      // onclose will fire after this
    }

    ws.onmessage = (event: MessageEvent) => {
      if (!mountedRef.current || ws !== conn.ws) return
      try {
        const parsed = JSON.parse(event.data) as Record<string, unknown>
        const prev = conn.turns
        const { turns: newTurns, terminal } = applyEvent(prev, parsed as TranscriptEvent)
        conn.turns = newTurns
        optionsRef.current.onTurnsUpdate?.(sid, [...newTurns])
        if (terminal) {
          optionsRef.current.onTerminal?.(sid)
        }
      } catch {
        // Ignore parse errors for malformed messages
      }
    }
  }

  // ── Public API ────────────────────────────────────────────────────────

  const waitForReady = useCallback(async (sid: string): Promise<void> => {
    const conn = connectionsRef.current.get(sid)
    if (!conn || conn.readyState === ReadyState.OPEN) return
    return new Promise<void>((resolve) => {
      conn.readyResolve = resolve
    })
  }, [])

  const sendPrompt = useCallback(
    async (sid: string, text: string): Promise<void> => {
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "prompt", text }))
      }
    },
    [waitForReady],
  )

  const interrupt = useCallback(
    async (sid: string): Promise<void> => {
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "interrupt" }))
      }
    },
    [waitForReady],
  )

  const respondToPermission = useCallback(
    async (sid: string, id: string, approved: boolean): Promise<void> => {
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "permission_response", id, approved }))
      }
    },
    [waitForReady],
  )

  const getTurns = useCallback((sid: string): TranscriptTurn[] => {
    return connectionsRef.current.get(sid)?.turns ?? []
  }, [])

  const isReady = useCallback((sid: string): boolean => {
    return connectionsRef.current.get(sid)?.readyState === ReadyState.OPEN
  }, [])

  return {
    sendPrompt,
    interrupt,
    respondToPermission,
    getTurns,
    isReady,
  }
}
