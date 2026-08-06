// ── Multi-session WebSocket agent transport hook ─────────────────────────
//
// Manages multiple WebSocket connections simultaneously, one per active
// session. Each connection independently processes events into its own turn
// list. Connections are created/removed as the sessionIds array changes.

import { useCallback, useEffect, useRef } from "react"
import { applyEvent, historyToTurns } from "./reducer"
import type { HistoryMessage, TranscriptEvent, TranscriptTurn } from "./types"
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
  /** Highest event `seq` received from the server. 0 = cold start (no dedup).
   *  Persisted across reconnects (the same `conn` is reused) so the WS URL
   *  carries `lastSeq` on reconnect and the server replays the gap. */
  lastSeq: number
}

function createConnectionState(): ConnectionState {
  return {
    ws: null,
    turns: [],
    readyState: ReadyState.UNINSTANTIATED,
    reconnectCount: 0,
    reconnectTimer: null,
    readyResolve: null,
    lastSeq: 0,
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

  // Track previous sessionIds to detect actual changes.
  const prevSessionIdsRef = useRef<string[]>([])

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
    // Resolve any pending waitForReady promise so callers don't hang forever.
    conn.readyResolve?.()
    conn.readyResolve = null
    conn.readyState = ReadyState.CLOSED
  }

  function connectSession(sid: string, conn: ConnectionState) {
    if (!mountedRef.current) return

    const lastSeqParam = conn.lastSeq > 0 ? `&lastSeq=${conn.lastSeq}` : ""
    const url = `${wsBaseUrl()}/api/ai/sessions/ws?id=${sid}${lastSeqParam}`
    conn.readyState = ReadyState.CONNECTING

    const ws = new WebSocket(url)
    conn.ws = ws

    ws.onopen = () => {
      if (!mountedRef.current || ws !== conn.ws) return
      conn.readyState = ReadyState.OPEN
      conn.reconnectCount = 0
      conn.readyResolve?.()
      conn.readyResolve = null
      // Cold start (lastSeq===0): load full history via REST, then accept all
      // live events (dedup off). Reconnect (lastSeq>0): the server replays
      // the gap from its ring buffer, so skip fetchHistory and dedup by seq.
      if (conn.lastSeq === 0) {
        fetchHistory(sid, conn)
      }
    }

    ws.onclose = () => {
      if (!mountedRef.current || ws !== conn.ws) return
      conn.readyState = ReadyState.CLOSED

      // Auto-reconnect with exponential backoff
      const maxAttempts = 20
      if (conn.reconnectCount < maxAttempts) {
        const delay = Math.min(1000 * Math.pow(1.5, conn.reconnectCount), 30000)
        conn.reconnectCount++
        // Clear any previous timer before setting a new one (prevents duplicate
        // reconnects when onclose fires multiple times).
        if (conn.reconnectTimer) clearTimeout(conn.reconnectTimer)
        conn.reconnectTimer = setTimeout(() => {
          // Check ws !== conn.ws: if the connection was torn down and recreated
          // during the delay, this stale timer must not start a new connection.
          if (mountedRef.current && ws === conn.ws) connectSession(sid, conn)
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
        // Resync signal: gap too large to replay — reset and reload full
        // history. Advance lastSeq past this signal and bail.
        if (
          parsed.kind === "session" &&
          parsed.type === "resync_required"
        ) {
          conn.turns = []
          optionsRef.current.onTurnsUpdate?.(sid, [])
          fetchHistory(sid, conn)
          const seq = Number(parsed.seq)
          if (Number.isFinite(seq) && seq > 0) conn.lastSeq = seq
          return
        }
        // Dedup by seq: drop events already seen (overlap of replay + live,
        // or a duplicate after reconnect). seq=0 (out-of-band, e.g. session
        // create-failure) is always accepted — it bypasses dedup.
        const seq = Number(parsed.seq)
        if (Number.isFinite(seq) && seq > 0) {
          if (seq <= conn.lastSeq) return
          conn.lastSeq = seq
        }
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

  // ── History loading ────────────────────────────────────────────────────

  async function fetchHistory(sid: string, conn: ConnectionState) {
    try {
      const base = getApiBaseUrl()
      const res = await fetch(`${base}/api/ai/sessions/${sid}/messages`)
      if (!res.ok) return
      const msgs: HistoryMessage[] = await res.json()
      if (!msgs || msgs.length === 0) return
      // Never overwrite a turn list that has an in-progress (streaming) turn —
      // live data always takes precedence over a stale history snapshot.
      // This matters when switching to a session that is still streaming in
      // the background: the live turns are authoritative until the turn ends.
      if (conn.turns.some((t) => t.status === "in_progress")) return
      const turns = historyToTurns(msgs)
      conn.turns = turns
      // Only notify if the connection is still alive and hasn't been
      // replaced by a newer connection.
      if (mountedRef.current) {
        optionsRef.current.onTurnsUpdate?.(sid, [...turns])
      }
    } catch {
      // History loading is best-effort; live events will still arrive.
    }
  }

  // Sync connections when sessionIds actually change.
  //
  // IMPORTANT: This effect has NO cleanup function. The cleanup would run on
  // every dependency change and tear down ALL connections before the new
  // effect runs, even when the quick check below decides nothing changed.
  // That caused mass disconnects of all sessions on every re-render where
  // the sessionIds array reference changed but contents were identical.
  //
  // Unmount-only teardown is handled by a separate useEffect([]) below.
  useEffect(() => {
    mountedRef.current = true
    const prev = prevSessionIdsRef.current
    prevSessionIdsRef.current = sessionIds

    const connections = connectionsRef.current
    const desired = new Set(sessionIds)
    const previous = new Set(prev)

    // Quick check: if nothing changed, skip entirely.
    if (desired.size === previous.size) {
      let same = true
      for (const sid of desired) {
        if (!previous.has(sid)) { same = false; break }
      }
      if (same) return
    }

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
  }, [sessionIds])

  // Unmount-only cleanup: tear down all connections when the component
  // unmounts. This is a separate effect with [] deps so it only runs once
  // on mount and cleans up on unmount — never on intermediate re-renders.
  useEffect(() => {
    return () => {
      mountedRef.current = false
      for (const [sid, conn] of connectionsRef.current) {
        teardownConnection(sid, conn)
      }
      connectionsRef.current.clear()
    }
  }, [])

  // ── Connection lifecycle helpers ──────────────────────────────────────

  // ── Public API ────────────────────────────────────────────────────────

  // Create the connection on demand if it's missing (e.g. sendPrompt raced
  // ahead of the sessionIds effect, or the connection was torn down). Makes
  // the send path self-healing instead of throwing "Session not found".
  const ensureConnection = useCallback((sid: string) => {
    let conn = connectionsRef.current.get(sid)
    if (!conn) {
      conn = createConnectionState()
      connectionsRef.current.set(sid, conn)
      connectSession(sid, conn)
    }
    return conn
  }, [])

  const waitForReady = useCallback(async (sid: string): Promise<void> => {
    // Poll until the connection exists (may not be created yet if the
    // caller just triggered a state update that will cause a re-render
    // and subsequent connection creation via the useEffect).
    let conn = connectionsRef.current.get(sid)
    if (!conn) {
      for (let i = 0; i < 50; i++) {
        await new Promise((r) => setTimeout(r, 50))
        conn = connectionsRef.current.get(sid)
        if (conn) break
      }
      if (!conn) {
        throw new Error(`Session ${sid} not found`)
      }
    }
    if (conn.readyState === ReadyState.OPEN) return
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        if (conn.readyResolve === resolve) conn.readyResolve = null
        reject(new Error(`Session ${sid} 连接超时`))
      }, 10000)
      conn.readyResolve = () => {
        clearTimeout(timer)
        resolve()
      }
    })
  }, [])

  const sendPrompt = useCallback(
    async (sid: string, text: string): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "prompt", text }))
      }
    },
    [ensureConnection, waitForReady],
  )

  const interrupt = useCallback(
    async (sid: string): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "interrupt" }))
      }
    },
    [ensureConnection, waitForReady],
  )

  const respondToPermission = useCallback(
    async (sid: string, id: string, approved: boolean): Promise<void> => {
      ensureConnection(sid)
      await waitForReady(sid)
      const conn = connectionsRef.current.get(sid)
      if (conn?.ws && conn.ws.readyState === WebSocket.OPEN) {
        conn.ws.send(JSON.stringify({ type: "permission_response", id, approved }))
      }
    },
    [ensureConnection, waitForReady],
  )

  const getTurns = useCallback((sid: string): TranscriptTurn[] => {
    return connectionsRef.current.get(sid)?.turns ?? []
  }, [])

  const isReady = useCallback((sid: string): boolean => {
    return connectionsRef.current.get(sid)?.readyState === ReadyState.OPEN
  }, [])

  /** Explicitly load history messages for a session from the REST API.
   *  Useful when switching to an already-connected session whose history
   *  was loaded only once during WebSocket onopen. */
  const loadHistory = useCallback(async (sid: string): Promise<void> => {
    const conn = connectionsRef.current.get(sid)
    if (!conn) return
    await fetchHistory(sid, conn)
  }, [])

  return {
    sendPrompt,
    interrupt,
    respondToPermission,
    getTurns,
    isReady,
    loadHistory,
  }
}
